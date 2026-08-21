use rust_mcp_macros::mcp_prompt;
use rust_mcp_schema::{ContentBlock, Prompt, Role};
use std::collections::BTreeMap;

#[test]
fn full_annotated_prompt() {
    #[mcp_prompt(
        name = "friendly-greeting",
        title = "Friendly Greeting",
        description = "Generate a warm, personalized greeting",
        meta = "{\"key\": \"value\"}",
        icons = [(src = "icon.png", mime_type = "image/png", sizes = ["128x128"])],
        messages = [
            (role = "user",
             content = "Write a short, warm greeting for {name}. Mention one thing that makes them awesome."),
        ]
    )]
    #[derive(Debug)]
    #[allow(dead_code)]
    struct FriendlyGreeting {
        #[prompt_argument(title = "Name", description = "Who to greet", default = "friend")]
        pub name: String,
        pub locale: Option<String>,
    }

    assert_eq!(FriendlyGreeting::prompt_name(), "friendly-greeting");

    let prompt: Prompt = FriendlyGreeting::prompt();
    assert_eq!(prompt.name, "friendly-greeting");
    assert_eq!(prompt.title.as_deref(), Some("Friendly Greeting"));
    assert_eq!(
        prompt.description.as_deref(),
        Some("Generate a warm, personalized greeting")
    );
    assert_eq!(prompt.meta.unwrap().get("key").unwrap(), "value");
    assert_eq!(prompt.icons.len(), 1);

    let args = FriendlyGreeting::prompt_arguments();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "name");
    assert_eq!(args[0].title.as_deref(), Some("Name"));
    assert_eq!(args[0].description.as_deref(), Some("Who to greet"));
    assert_eq!(args[0].required, None); // has a default -> optional
    assert_eq!(args[1].name, "locale");
    assert_eq!(args[1].required, None);

    let params = FriendlyGreeting::request_params();
    assert_eq!(params.name, "friendly-greeting");
    assert!(params.arguments.is_none());
}

#[test]
fn from_arguments_applies_defaults_and_validates_required() {
    #[mcp_prompt(
        name = "greet",
        messages = [
            (role = "user", content = "Hello {name}!"),
            (role = "assistant", content = "Hi {name}, nice to meet you."),
        ]
    )]
    #[derive(Debug)]
    struct Greet {
        #[prompt_argument(default = "friend")]
        pub name: String,
        pub subject: String,
    }

    let mut args = BTreeMap::new();
    args.insert("subject".to_string(), "Rust".to_string());

    let greet = Greet::from_arguments(Some(&args)).unwrap();
    let result = greet.render();
    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[0].role, Role::User);
    assert_eq!(result.messages[1].role, Role::Assistant);

    match &result.messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello friend!"),
        _ => panic!("expected text content"),
    }

    // Missing required argument -> error.
    let err = Greet::from_arguments(None).unwrap_err();
    assert!(err.message.contains("subject"));
}

#[test]
fn declaration_only_prompt_without_messages() {
    #[mcp_prompt(
        name = "declaration-only",
        title = "Declaration Only",
        description = "A prompt declared without message templates."
    )]
    #[allow(dead_code)]
    struct DeclarationOnly {
        #[prompt_argument(description = "Some input")]
        pub input: String,
    }

    assert_eq!(DeclarationOnly::prompt_name(), "declaration-only");

    let prompt: Prompt = DeclarationOnly::prompt();
    assert_eq!(prompt.name, "declaration-only");
    assert_eq!(prompt.title.as_deref(), Some("Declaration Only"));
    assert_eq!(
        prompt.description.as_deref(),
        Some("A prompt declared without message templates.")
    );
    assert_eq!(prompt.arguments.len(), 1);
    assert_eq!(prompt.arguments[0].name, "input");
}

#[test]
fn serde_rename_maps_argument_name() {
    #[mcp_prompt(
        name = "rename-greet",
        messages = [(role = "user", content = "Hello {who}!")]
    )]
    #[derive(::serde::Serialize, ::serde::Deserialize)]
    #[allow(dead_code)]
    struct RenameGreet {
        #[serde(rename = "who")]
        #[prompt_argument(default = "friend")]
        pub name: String,
    }

    let args = RenameGreet::prompt_arguments();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "who");

    let result = RenameGreet::from_arguments(None).unwrap().render();
    match &result.messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello friend!"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn argument_required_is_derived_from_type() {
    #[mcp_prompt(
        name = "overrides",
        messages = [(role = "user", content = "{a} {b} {c}")]
    )]
    #[allow(dead_code)]
    struct Overrides {
        pub a: String,
        pub b: Option<String>,
        #[prompt_argument(default = "x")]
        pub c: String,
    }

    let args = Overrides::prompt_arguments();
    assert_eq!(args[0].name, "a");
    assert_eq!(args[0].required, Some(true)); // String -> required
    assert_eq!(args[1].name, "b");
    assert_eq!(args[1].required, None); // Option<String> -> optional
    assert_eq!(args[2].name, "c");
    assert_eq!(args[2].required, None); // String with default -> optional
}

#[test]
fn optional_argument_without_default_interpolates_empty() {
    #[mcp_prompt(
        name = "greet",
        messages = [(role = "user", content = "Hello{name}!")]
    )]
    struct Greet {
        pub name: Option<String>,
    }

    let result = Greet::from_arguments(None).unwrap().render();
    match &result.messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello!"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn extra_arguments_ignored_and_placeholders_repeated() {
    #[mcp_prompt(
        name = "greet",
        messages = [(role = "user", content = "{name} {name} {name}")]
    )]
    struct Greet {
        pub name: String,
    }

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Ali".to_string());
    args.insert("unused".to_string(), "ignored".to_string());

    let result = Greet::from_arguments(Some(&args)).unwrap().render();
    match &result.messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Ali Ali Ali"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn concat_multiline_attributes() {
    #[mcp_prompt(
        name = concat!("multi", "-", "line"),
        title = concat!("Multi", " Line"),
        description = concat!("A ", "multi-line ", "description"),
        messages = [
            (role = "user",
             content = concat!("Hello ", "there, ", "{name}! ", "How are you?")),
        ]
    )]
    #[allow(dead_code)]
    struct MultiLine {
        #[prompt_argument(
            title = concat!("Arg ", "Title"),
            description = concat!("A ", "long ", "description"),
            default = concat!("fri", "end"),
        )]
        pub name: String,
    }

    assert_eq!(MultiLine::prompt_name(), "multi-line");

    let prompt = MultiLine::prompt();
    assert_eq!(prompt.title.as_deref(), Some("Multi Line"));
    assert_eq!(
        prompt.description.as_deref(),
        Some("A multi-line description")
    );

    let args = MultiLine::prompt_arguments();
    assert_eq!(args[0].title.as_deref(), Some("Arg Title"));
    assert_eq!(args[0].description.as_deref(), Some("A long description"));

    let result = MultiLine::from_arguments(None).unwrap().render();
    match &result.messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello there, friend! How are you?"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn prompt_name_const_title_and_meta_accessors() {
    #[mcp_prompt(
        name = "accessors",
        title = "Accessors Title",
        description = "Accessors description",
        meta = "{\"k\": \"v\"}",
        messages = [(role = "user", content = "Hello!")]
    )]
    #[allow(dead_code)]
    struct Accessors {}

    assert_eq!(Accessors::PROMPT_NAME, "accessors");
    assert_eq!(Accessors::prompt_name(), "accessors");
    assert_eq!(Accessors::prompt_title(), Some("Accessors Title"));
    assert_eq!(
        Accessors::prompt_description(),
        Some("Accessors description")
    );
    assert_eq!(Accessors::prompt_meta(), Some("{\"k\": \"v\"}"));

    // PROMPT_NAME is a const and can be used directly in a match pattern.
    let matched = match "accessors" {
        Accessors::PROMPT_NAME => "yes",
        _ => "no",
    };
    assert_eq!(matched, "yes");
}

#[test]
fn accessors_return_none_when_not_declared() {
    #[mcp_prompt(
        name = "bare",
        messages = [(role = "user", content = "Hi")]
    )]
    #[allow(dead_code)]
    struct Bare {}

    assert_eq!(Bare::PROMPT_NAME, "bare");
    assert_eq!(Bare::prompt_title(), None);
    assert_eq!(Bare::prompt_description(), None);
    assert_eq!(Bare::prompt_meta(), None);
}
