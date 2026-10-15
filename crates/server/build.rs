fn main() {
    // Adding a migration must also invalidate sqlx::migrate!'s embedded list.
    println!("cargo:rerun-if-changed=../../migrations");
}
