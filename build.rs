fn main() {
    let config_dir = dirs::state_dir()
        .or_else(|| dirs::config_dir())
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
        .expect("Could not find config directory");
    
    let db_path = config_dir.join("todo").join("data");
    std::fs::create_dir_all(&db_path).expect("Failed to create database directory");
    
    let db_file = db_path.join("todo.db");
    let database_url = format!("sqlite:{}", db_file.display());
    
    println!("cargo:rustc-env=DATABASE_URL={}", database_url);
}