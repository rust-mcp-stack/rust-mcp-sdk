use rust_mcp_macros::{mcp_elicit, mcp_resource, JsonSchema};
use rust_mcp_schema::{Resource, Role};

#[test]
fn full_annotated_resource() {
    #[
        mcp_resource(
        name = "my-resource",
        uri = "https://example.com/file.pdf",
        description = "Important document",
        title = "My Document",
        meta = "{\"key\": \"value\", \"num\": 42}",
        mime_type = "application/pdf",
        size = 1024,
        audience = ["user", "assistant"],
        icons = [(src = "icon.png", mime_type = "image/png", sizes = ["48x48"])],
    )
    ]
    struct MyResource {
        pub api_key: String,
    }

    let resource: Resource = MyResource::resource();
    assert_eq!(resource.name, "my-resource");
    assert_eq!(resource.uri, "https://example.com/file.pdf");
    assert_eq!(resource.description.unwrap(), "Important document");
    assert_eq!(resource.title.unwrap(), "My Document");
    assert_eq!(resource.mime_type.unwrap(), "application/pdf");
    assert_eq!(resource.size.unwrap(), 1024);
    assert_eq!(
        resource.annotations.unwrap().audience,
        vec![Role::User, Role::Assistant]
    );
    assert_eq!(resource.icons.len(), 1);
    let icon = &resource.icons[0];
    assert_eq!(icon.mime_type.as_ref().unwrap(), "image/png");
    assert_eq!(icon.src, "icon.png");
    assert_eq!(icon.theme, None);
    assert_eq!(icon.sizes, vec!["48x48"]);
}
