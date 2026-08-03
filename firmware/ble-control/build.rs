// esp-idf build glue: emit the environment the linker/bindings need.
fn main() {
    embuild::espidf::sysenv::output();
}
