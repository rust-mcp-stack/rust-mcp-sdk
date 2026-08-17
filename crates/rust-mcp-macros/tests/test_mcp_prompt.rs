use rust_mcp_macros::mcp_prompt;
use rust_mcp_schema::{ContentBlock, GetPromptRequestParams, Prompt, Role};
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
        #[prompt_argument(required = false)]
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
    assert_eq!(args[0].required, Some(true));
    assert_eq!(args[1].name, "locale");
    assert_eq!(args[1].required, None);

    let params = FriendlyGreeting::request_params();
    assert_eq!(params.name, "friendly-greeting");
    assert!(params.arguments.is_none());
}

#[test]
fn render_prompt_applies_defaults_and_validates_required() {
    #[mcp_prompt(
        name = "greet",
        messages = [
            (role = "user", content = "Hello {name}!"),
            (role = "assistant", content = "Hi {name}, nice to meet you."),
        ]
    )]
    #[allow(dead_code)]
    struct Greet {
        #[prompt_argument(default = "friend")]
        pub name: String,
        #[prompt_argument(required = true)]
        pub subject: String,
    }

    let mut args = BTreeMap::new();
    args.insert("subject".to_string(), "Rust".to_string());

    let messages = Greet::render_prompt(Some(args)).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Assistant);

    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello friend!"),
        _ => panic!("expected text content"),
    }

    // Missing required argument -> error.
    let err = Greet::render_prompt(None).unwrap_err();
    assert!(err.message.contains("subject"));
}

#[test]
fn get_prompt_result_rejects_unknown_name() {
    #[mcp_prompt(
        name = "greet",
        description = "A greeting",
        messages = [(role = "user", content = "Hello!")]
    )]
    #[allow(dead_code)]
    struct Greet {}

    let params = GetPromptRequestParams {
        name: "unknown".to_string(),
        arguments: None,
        meta: None,
    };

    let err = Greet::get_prompt_result(params).unwrap_err();
    assert!(err.message.contains("Unknown prompt"));

    let ok = Greet::get_prompt_result(GetPromptRequestParams {
        name: "greet".to_string(),
        arguments: None,
        meta: None,
    })
    .unwrap();
    assert_eq!(ok.messages.len(), 1);
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

    let messages = RenameGreet::render_prompt(None).unwrap();
    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello friend!"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn explicit_required_overrides_option_fallback() {
    #[mcp_prompt(
        name = "overrides",
        messages = [(role = "user", content = "{a} {b} {c} {d}")]
    )]
    #[allow(dead_code)]
    struct Overrides {
        #[prompt_argument(required = true)]
        pub a: Option<String>,
        #[prompt_argument(required = false)]
        pub b: String,
        pub c: String,
        pub d: Option<String>,
    }

    let args = Overrides::prompt_arguments();
    assert_eq!(args[0].name, "a");
    assert_eq!(args[0].required, Some(true)); // Option but explicitly required
    assert_eq!(args[1].name, "b");
    assert_eq!(args[1].required, None); // String but explicitly optional
    assert_eq!(args[2].required, Some(true)); // String fallback -> required
    assert_eq!(args[3].required, None); // Option fallback -> optional
}

#[test]
fn optional_argument_without_default_interpolates_empty() {
    #[mcp_prompt(
        name = "greet",
        messages = [(role = "user", content = "Hello{name}!")]
    )]
    #[allow(dead_code)]
    struct Greet {
        pub name: Option<String>,
    }

    let messages = Greet::render_prompt(None).unwrap();
    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello!"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn default_applied_to_optional_field() {
    #[mcp_prompt(
        name = "greet",
        messages = [(role = "user", content = "Hello {name}!")]
    )]
    #[allow(dead_code)]
    struct Greet {
        #[prompt_argument(default = "friend")]
        pub name: Option<String>,
    }

    let messages = Greet::render_prompt(None).unwrap();
    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello friend!"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn extra_arguments_ignored_and_placeholders_repeated() {
    #[mcp_prompt(
        name = "greet",
        messages = [(role = "user", content = "{name} {name} {name}")]
    )]
    #[allow(dead_code)]
    struct Greet {
        pub name: String,
    }

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Ali".to_string());
    args.insert("unused".to_string(), "ignored".to_string());

    let messages = Greet::render_prompt(Some(args)).unwrap();
    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Ali Ali Ali"),
        _ => panic!("expected text content"),
    }
}

#[test]
fn get_prompt_result_includes_description_and_meta() {
    #[mcp_prompt(
        name = "greet",
        description = "A friendly greeting",
        meta = "{\"k\": \"v\"}",
        messages = [(role = "user", content = "Hello!")]
    )]
    #[allow(dead_code)]
    struct Greet {}

    let ok = Greet::get_prompt_result(GetPromptRequestParams {
        name: "greet".to_string(),
        arguments: None,
        meta: None,
    })
    .unwrap();

    assert_eq!(ok.description.as_deref(), Some("A friendly greeting"));
    assert_eq!(ok.meta.unwrap().get("k").unwrap(), "v");
    assert_eq!(ok.messages.len(), 1);
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

    let messages = MultiLine::render_prompt(None).unwrap();
    match &messages[0].content {
        ContentBlock::TextContent(tc) => assert_eq!(tc.text, "Hello there, friend! How are you?"),
        _ => panic!("expected text content"),
    }
}
