pub trait Prompt {
    fn load(name: &str) -> String;

    fn prompt(&self) -> String;
}
