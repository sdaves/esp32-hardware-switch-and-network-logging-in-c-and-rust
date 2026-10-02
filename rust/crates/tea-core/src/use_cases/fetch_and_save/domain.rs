#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    HttpResponseReady { success: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    SaveIso8601File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub saved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
