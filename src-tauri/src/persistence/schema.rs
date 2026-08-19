diesel::table! {
    conversations (id) {
        id -> Text,
        title -> Text,
        default_provider_profile_id -> Text,
        default_model -> Text,
        archived -> Bool,
        version -> BigInt,
        created_at_ms -> BigInt,
        updated_at_ms -> BigInt,
    }
}

diesel::table! {
    provider_profiles (id) {
        id -> Text,
        label -> Text,
        kind -> Text,
        protocol -> Text,
        base_url -> Text,
        default_model -> Text,
        credential_ref -> Text,
        store_responses -> Bool,
        supports_tools -> Bool,
        supports_images -> Bool,
        supports_files -> Bool,
        config_json -> Text,
        created_at_ms -> BigInt,
        updated_at_ms -> BigInt,
    }
}
