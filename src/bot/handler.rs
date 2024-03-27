pub trait ChatHandler {
    fn on_event(&self, event: &ChatEvent);
}

pub enum ChatEvent {
    Delta(String),
    Error(String),
    End,
}
