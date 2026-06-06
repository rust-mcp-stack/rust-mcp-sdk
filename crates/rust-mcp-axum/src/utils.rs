pub(crate) fn remove_query_and_hash(endpoint: &str) -> String {
    let without_fragment = endpoint.split_once('#').map_or(endpoint, |(path, _)| path);
    let without_query = without_fragment
        .split_once('?')
        .map_or(without_fragment, |(path, _)| path);
    if without_query.is_empty() {
        "/".to_string()
    } else {
        without_query.to_string()
    }
}
