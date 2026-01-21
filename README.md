# rs-bean

A lightweight Rust library for bean management and dependency injection.

> ⚠️ APIs are not stable yet; future releases may introduce breaking changes.

## Features

- **Dependency Injection**: Automatic dependency resolution and injection
- **Scope Management**: Support for Singleton and Prototype scopes
- **Circular Dependency Detection**: Detects and prevents circular dependencies with detailed error messages
- **Type-safe**: Leverages Rust's type system for compile-time safety
- **Thread-safe**: Built with `Arc` and `RwLock` for concurrent access
- **Named Beans**: Register multiple beans of the same type with different names
- **Zero External Dependencies**: Pure Rust implementation using only std library

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rs-bean = "0.1.0"
```

## Quick Start

```rust
use std::sync::Arc;

use rs_bean::bean::{BeanContainer, Scope};

// Define your services
struct Database {
    connection_string: String,
}

impl Database {
    fn new(connection_string: String) -> Self {
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

    // Use the container
    let order_service = container.get::<OrderService>()?;
    order_service.create_order(1, "Laptop");

    // Singleton scope reuses instances
    let order_service2 = container.get::<OrderService>()?;
    assert!(Arc::ptr_eq(&order_service, &order_service2));

    Ok(())
}
```

## Usage

### Registering Beans

#### Default Registration (by Type)

```rust
// Singleton scope - single instance shared across all requests
container.register::<MyService, _>(Scope::Singleton, |deps| {
    Ok(MyService::new())
})?;

// Prototype scope - new instance created for each request
container.register::<MyService, _>(Scope::Prototype, |deps| {
    Ok(MyService::new())
})?;
```

#### Named Registration

```rust
// Register multiple beans of the same type with different names
container.register_named::<Database, _>("primary-db", Scope::Singleton, |_deps| {
    Ok(Database::new("postgresql://primary:5432/db"))
})?;

container.register_named::<Database, _>("replica-db", Scope::Singleton, |_deps| {
    Ok(Database::new("postgresql://replica:5432/db"))
})?;
```

### Retrieving Beans

#### Get by Type

```rust
let service = container.get::<MyService>()?;
```

#### Get by Name

```rust
let primary_db = container.get_named::<Database>("primary-db")?;
let replica_db = container.get_named::<Database>("replica-db")?;
```

### Dependency Injection

Dependencies are automatically resolved during bean creation:

```rust
container.register::<Database, _>(Scope::Singleton, |_deps| {
    Ok(Database::new("postgresql://localhost:5432/mydb"))
})?;

container.register::<UserRepository, _>(Scope::Singleton, |deps| {
    let db = deps.get::<Database>()?;  // Auto-injected
    Ok(UserRepository::new(db))
})?;

container.register::<UserService, _>(Scope::Singleton, |deps| {
    let repo = deps.get::<UserRepository>()?;  // Auto-injected
    Ok(UserService::new(repo))
})?;
```

### Circular Dependency Detection

The container automatically detects circular dependencies:

```rust
// This will fail with a clear error message
container.register::<ServiceA, _>(Scope::Singleton, |deps| {
    let b = deps.get::<ServiceB>()?;  // ServiceA depends on ServiceB
    Ok(ServiceA::new(b))
})?;

container.register::<ServiceB, _>(Scope::Singleton, |deps| {
    let a = deps.get::<ServiceA>()?;  // ServiceB depends on ServiceA - CIRCULAR!
    Ok(ServiceB::new(a))
})?;

// Error: Circular dependency detected! Dependency path: Bean(ServiceA) -> Bean(ServiceB) -> Bean(ServiceA)
```

## API Reference

### `BeanContainer`

The main container for managing beans.

#### Methods

- `new() -> Self` - Create a new bean container
- `register<T, F>(scope: Scope, factory: F) -> Result<(), String>` - Register a bean by type
- `register_named<T, F>(name: &str, scope: Scope, factory: F) -> Result<(), String>` - Register a named bean
- `get<T>() -> Result<Arc<T>, String>` - Get a bean by type
- `get_named<T>(name: &str) -> Result<Arc<T>, String>` - Get a bean by name
- `contains<T>(name: Option<&str>) -> bool` - Check if a bean exists
- `len() -> usize` - Get the number of registered beans
- `is_empty() -> bool` - Check if the container is empty

### `Scope`

Bean lifecycle scope.

- `Scope::Singleton` - Single instance shared across all requests
- `Scope::Prototype` - New instance created for each request

### `Dependencies`

Provides access to other beans during bean creation.

#### Methods

- `get<T>() -> Result<Arc<T>, String>` - Get a dependency by type
- `get_named<T>(name: Option<&str>) -> Result<Arc<T>, String>` - Get a dependency by name
- `current_path() -> String` - Get the current dependency resolution path (for debugging)

## Examples

See the [examples](examples/) directory for more detailed examples:

- [basic.rs](examples/basic.rs) - Basic usage with dependency injection

Run examples with:

```bash
cargo run --example basic
```

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
