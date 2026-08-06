#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptParameter {
    pub name: String,
    pub repeatable: bool,
    pub pack_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptInjection {
    pub value: String,
    pub tag: Option<String>,
}
