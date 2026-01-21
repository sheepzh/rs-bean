use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scope {
    Singleton,
    Prototype,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Identifier {
    Named(String),    // Named Bean
    TypeSpec(TypeId), // Type-specific default Bean
    Unnamed(TypeId),  // Unnamed temporary Bean (replaced by TypeSpec)
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identifier::Named(name) => write!(f, "Bean({})", name),
            Identifier::TypeSpec(type_id) => write!(f, "Bean({:?})", type_id),
            Identifier::Unnamed(type_id) => write!(f, "Bean({:?})[unnamed]", type_id),
        }
    }
}

/// Creation context
struct CreationContext {
    // Creation stack
    creating: Vec<Identifier>,
}

impl CreationContext {
    fn new() -> Self {
        CreationContext {
            creating: Vec::new(),
        }
    }

    fn enter(&mut self, id: Identifier) -> Result<(), String> {
        if self.creating.len() > 100 {
            return Err("Dependency chain too deep (>100)".to_string());
        }

        // Check for circular dependencies
        if self.creating.iter().any(|i| i == &id) {
            let path = self
                .creating
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(format!("Circular dependency detected: {} -> {}", path, id));
        }

        self.creating.push(id);
        Ok(())
    }

    fn exit(&mut self) {
        self.creating.pop();
    }

    /// get current dependency path
    pub fn get_path(&self) -> String {
        self.creating
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

/// Dependency provider
pub struct Dependencies<'a> {
    container: &'a BeanContainer,
    context: &'a mut CreationContext,
}

impl<'a> Dependencies<'a> {
    /// Get bean with default name
    pub fn get<T: Any + Send + Sync + 'static>(&mut self) -> Result<Arc<T>, String> {
        self.get_named::<T>(None)
    }

    /// Get bean with specified name
    pub fn get_named<T: Any + Send + Sync + 'static>(
        &mut self,
        name: Option<&str>,
    ) -> Result<Arc<T>, String> {
        self.container.get_with_context::<T>(name, self.context)
    }

    /// Get current dependency path (for debugging)
    pub fn current_path(&self) -> String {
        self.context.get_path()
    }
}

pub trait BeanFactory: Send + Sync {
    fn create(&self, deps: &mut Dependencies) -> Result<Arc<dyn Any + Send + Sync>, String>;
}

struct BeanDefinition {
    factory: Arc<dyn BeanFactory>,
    scope: Scope,
    instance: Option<Arc<dyn Any + Send + Sync>>,
}

pub struct BeanContainer {
    beans: RwLock<HashMap<Identifier, BeanDefinition>>,
}

impl BeanContainer {
    pub fn new() -> Self {
        BeanContainer {
            beans: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T, F>(&self, scope: Scope, factory: F) -> Result<(), String>
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&mut Dependencies) -> Result<T, String> + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_spec_id = Identifier::TypeSpec(type_id);
        let unnamed_id = Identifier::Unnamed(type_id);

        let bean_factory: Arc<dyn BeanFactory> = Arc::new(move |deps: &mut Dependencies| {
            let instance = factory(deps)?;
            Ok(Arc::new(instance) as Arc<dyn Any + Send + Sync>)
        });

        let definition = BeanDefinition {
            factory: bean_factory,
            scope,
            instance: None,
        };

        let mut beans = self.beans.write().unwrap();

        // If TypeSpec exists, throw error
        if beans.contains_key(&type_spec_id) {
            return Err(format!("Bean already registered for type: {:?}", type_id));
        }
        // If unnamed exists, remove it
        beans.remove(&unnamed_id);
        // Add TypeSpec
        beans.insert(type_spec_id, definition);

        Ok(())
    }

    pub fn register_named<T, F>(&self, name: &str, scope: Scope, factory: F) -> Result<(), String>
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&mut Dependencies) -> Result<T, String> + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let named_id = Identifier::Named(name.to_string());
        let type_spec_id = Identifier::TypeSpec(type_id);
        let unnamed_id = Identifier::Unnamed(type_id);

        let bean_factory: Arc<dyn BeanFactory> = Arc::new(move |deps: &mut Dependencies| {
            let instance = factory(deps)?;
            Ok(Arc::new(instance) as Arc<dyn Any + Send + Sync>)
        });

        let definition = BeanDefinition {
            factory: bean_factory.clone(),
            scope,
            instance: None,
        };

        let mut beans = self.beans.write().unwrap();

        // Check if Named already exists
        if beans.contains_key(&named_id) {
            return Err(format!("Bean already registered with name: {}", name));
        }

        // Register Named
        beans.insert(named_id, definition);

        // Rule 1: If TypeSpec and Unnamed do not exist, add Unnamed
        if !beans.contains_key(&type_spec_id) && !beans.contains_key(&unnamed_id) {
            let unnamed_definition = BeanDefinition {
                factory: bean_factory,
                scope,
                instance: None,
            };
            beans.insert(unnamed_id, unnamed_definition);
        }

        Ok(())
    }

    /// Get bean by type
    pub fn get<T: Any + Send + Sync + 'static>(&self) -> Result<Arc<T>, String> {
        self.get_inner::<T>(None)
    }

    /// Get named bean by type
    pub fn get_named<T: Any + Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>, String> {
        self.get_inner::<T>(Some(name))
    }

    fn get_inner<T: Any + Send + Sync + 'static>(
        &self,
        name: Option<&str>,
    ) -> Result<Arc<T>, String> {
        let mut context = CreationContext::new();
        self.get_with_context::<T>(name, &mut context)
    }

    fn get_with_context<T: Any + Send + Sync + 'static>(
        &self,
        name: Option<&str>,
        context: &mut CreationContext,
    ) -> Result<Arc<T>, String> {
        let type_id = TypeId::of::<T>();

        // Determine the key to look up
        let id = if let Some(n) = name {
            Identifier::Named(n.to_string())
        } else {
            // Prefer TypeSpec, then Unnamed
            let type_spec_id = Identifier::TypeSpec(type_id);
            let unnamed_id = Identifier::Unnamed(type_id);

            let beans = self.beans.read().unwrap();
            if beans.contains_key(&type_spec_id) {
                type_spec_id
            } else if beans.contains_key(&unnamed_id) {
                unnamed_id
            } else {
                return Err(format!("Bean not found for type: {:?}", type_id));
            }
        };

        // Check for circular dependencies
        context.enter(id.clone())?;

        // Check if singleton is already created
        {
            let beans = self.beans.read().unwrap();
            if let Some(definition) = beans.get(&id)
                && definition.scope == Scope::Singleton
                && let Some(inst) = &definition.instance
            {
                return inst
                    .clone()
                    .downcast::<T>()
                    .map_err(|_| "Type downcast failed".to_string());
            }
        }

        let result = (|| -> Result<Arc<T>, String> {
            let (factory, scope) = {
                let beans = self.beans.read().unwrap();
                let definition = beans
                    .get(&id)
                    .ok_or_else(|| format!("Bean not found: {}", id))?;

                if definition.scope == Scope::Singleton
                    && let Some(inst) = &definition.instance
                {
                    return inst
                        .clone()
                        .downcast::<T>()
                        .map_err(|_| "Type downcast failed".to_string());
                }

                (definition.factory.clone(), definition.scope)
            };

            let mut deps = Dependencies {
                container: self,
                context,
            };
            let new_instance = factory.create(&mut deps)?;

            if scope == Scope::Singleton {
                let mut beans = self.beans.write().unwrap();
                if let Some(definition) = beans.get_mut(&id)
                    && definition.instance.is_none()
                {
                    definition.instance = Some(new_instance.clone());
                }
            }

            new_instance
                .downcast::<T>()
                .map_err(|_| "Type downcast failed".to_string())
        })();

        context.exit();
        result
    }

    /// Check if the container contains the specified bean
    pub fn contains<T: Any + Send + Sync + 'static>(&self, name: Option<&str>) -> bool {
        let type_id = TypeId::of::<T>();
        let beans = self.beans.read().unwrap();

        if let Some(n) = name {
            beans.contains_key(&Identifier::Named(n.to_string()))
        } else {
            beans.contains_key(&Identifier::TypeSpec(type_id))
                || beans.contains_key(&Identifier::Unnamed(type_id))
        }
    }

    /// Get the number of registered beans
    pub fn len(&self) -> usize {
        self.beans.read().unwrap().len()
    }

    /// Check if the container is empty
    pub fn is_empty(&self) -> bool {
        self.beans.read().unwrap().is_empty()
    }
}

impl Default for BeanContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> BeanFactory for F
where
    F: Fn(&mut Dependencies) -> Result<Arc<dyn Any + Send + Sync>, String> + Send + Sync,
{
    fn create(&self, deps: &mut Dependencies) -> Result<Arc<dyn Any + Send + Sync>, String> {
        self(deps)
    }
}
