use std::sync::Arc;

use rs_bean::bean::{BeanContainer, Scope};

// Define some simple structs
struct Database {
    connection_string: String,
}

impl Database {
    fn new(connection_string: String) -> Self {
        println!("Creating Database with connection: {}", connection_string);
        Database { connection_string }
    }

    fn query(&self, sql: &str) {
        println!("Executing query on {}: {}", self.connection_string, sql);
    }
}

struct UserService {
    db: Arc<Database>,
}

impl UserService {
    fn new(db: Arc<Database>) -> Self {
        println!("Creating UserService");
        UserService { db }
    }

    fn get_user(&self, id: u32) {
        self.db
            .query(&format!("SELECT * FROM users WHERE id = {}", id));
    }
}

struct OrderService {
    db: Arc<Database>,
    user_service: Arc<UserService>,
}

impl OrderService {
    fn new(db: Arc<Database>, user_service: Arc<UserService>) -> Self {
        println!("Creating OrderService");
        OrderService { db, user_service }
    }

    fn create_order(&self, user_id: u32, product: &str) {
        self.user_service.get_user(user_id);
        self.db.query(&format!(
            "INSERT INTO orders (user_id, product) VALUES ({}, '{}')",
            user_id, product
        ));
    }
}

fn main() -> Result<(), String> {
    // Create container
    let container = BeanContainer::new();

    // Register Database (Singleton)
    container.register::<Database, _>(Scope::Singleton, |_deps| {
        Ok(Database::new(
            "postgresql://localhost:5432/mydb".to_string(),
        ))
    })?;

    // Register UserService (Singleton)
    container.register::<UserService, _>(Scope::Singleton, |deps| {
        let db = deps.get::<Database>()?;
        Ok(UserService::new(db))
    })?;

    // Register OrderService (Singleton)
    container.register::<OrderService, _>(Scope::Singleton, |deps| {
        let db = deps.get::<Database>()?;
        let user_service = deps.get::<UserService>()?;
        Ok(OrderService::new(db, user_service))
    })?;

    println!("\n=== Get and use OrderService ===");
    let order_service = container.get::<OrderService>();
    order_service.create_order(1, "Laptop");

    println!("\n=== Get OrderService again (should reuse singleton) ===");
    let order_service2 = container.get::<OrderService>();
    order_service2.create_order(2, "Phone");

    println!("\n=== Verify same instance ===");
    println!(
        "Same instance: {}",
        Arc::ptr_eq(&order_service, &order_service2)
    );

    Ok(())
}
