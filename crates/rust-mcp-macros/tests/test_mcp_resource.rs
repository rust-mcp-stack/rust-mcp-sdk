use rust_mcp_macros::{mcp_elicit, mcp_resource, mcp_resource_template, JsonSchema};
use rust_mcp_schema::{Resource, ResourceTemplate, Role};

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
    assert_eq!(MyResource::resource_mime_type(), Some("application/pdf"));
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

#[test]
fn resource_uri_const_usable_in_match() {
    #[mcp_resource(
        name = "my-resource",
        uri = "https://example.com/file.pdf",
        description = "Important document"
    )]
    struct MyResource;

    assert_eq!(MyResource::RESOURCE_URI, "https://example.com/file.pdf");
    assert_eq!(MyResource::resource_uri(), MyResource::RESOURCE_URI);
    assert_eq!(MyResource::resource_mime_type(), None);

    let uri = "https://example.com/file.pdf";
    let matched = match uri {
        MyResource::RESOURCE_URI => "logo",
        _ => "other",
    };
    assert_eq!(matched, "logo");
}

#[test]
fn resource_uri_template_const_usable_in_match() {
    #[mcp_resource_template(
        name = "my-resource-template",
        uri_template = "https://example.com/{path}",
        description = "Important document"
    )]
    struct MyTemplate;

    assert_eq!(
        MyTemplate::RESOURCE_URI_TEMPLATE,
        "https://example.com/{path}"
    );
    assert_eq!(
        MyTemplate::resource_template_uri(),
        MyTemplate::RESOURCE_URI_TEMPLATE
    );
    assert_eq!(MyTemplate::resource_template_mime_type(), None);

    let uri = "https://example.com/{path}";
    let matched = match uri {
        MyTemplate::RESOURCE_URI_TEMPLATE => "template",
        _ => "other",
    };
    assert_eq!(matched, "template");
}

#[test]
fn full_annotated_resource_template() {
    #[
        mcp_resource_template(
        name = "my-resource-template",
        uri_template = "https://example.com/{path}",
        description = "Important document",
        title = "My Document",
        meta = "{\"key\": \"value\", \"num\": 42}",
        mime_type = "application/pdf",
        audience = ["user", "assistant"],
        icons = [(src = "icon.png", mime_type = "image/png", sizes = ["48x48"])],
    )
    ]
    struct MyResource {
        pub api_key: String,
    }

    let resource: ResourceTemplate = MyResource::resource_template();
    assert_eq!(resource.name, "my-resource-template");
    assert_eq!(resource.uri_template, "https://example.com/{path}");
    assert_eq!(resource.description.unwrap(), "Important document");
    assert_eq!(resource.title.unwrap(), "My Document");
    assert_eq!(resource.mime_type.unwrap(), "application/pdf");
    assert_eq!(
        MyResource::resource_template_mime_type(),
        Some("application/pdf")
    );
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

#[test]
fn adhoc() {
    use rust_mcp_macros::mcp_resource;
    #[mcp_resource_template(
        name = "company-logos",
        description = "The official company logos in different resolutions",
        title = "Company Logos",
        mime_type = "image/png",
        uri_template = "https://example.com/assets/{file_path}",
        audience = ["user", "assistant"],
        meta = "{\"license\": \"proprietary\", \"author\": \"Ali Hashemi\"}",
        icons = [
        ( src = "logo-192.png", sizes = ["192x192"], mime_type = "image/png" ),
        ( src = "logo-512.png", sizes = ["512x512"], mime_type = "image/png" )
        ]
    )]
    struct CompanyLogo {};

    // Usage
    assert_eq!(CompanyLogo::resource_template_name(), "company-logos");
    assert_eq!(
        CompanyLogo::resource_template_uri(),
        "https://example.com/assets/{file_path}"
    );

    let resource_template = CompanyLogo::resource_template();
    assert_eq!(resource_template.name, "company-logos");
    assert_eq!(resource_template.mime_type.unwrap(), "image/png");
    assert!(resource_template.icons.len() == 2);
}
