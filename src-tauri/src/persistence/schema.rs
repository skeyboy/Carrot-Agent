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
    runs (id) {
        id -> Text,
        conversation_id -> Text,
        parent_run_id -> Nullable<Text>,
        status -> Text,
        phase -> Text,
        strategy -> Text,
        provider_profile_id -> Text,
        provider_snapshot_json -> Text,
        model -> Text,
        version -> BigInt,
        last_event_seq -> BigInt,
        runtime_instance_id -> Nullable<Text>,
        lease_expires_at_ms -> Nullable<BigInt>,
        stop_reason -> Nullable<Text>,
        created_at_ms -> BigInt,
        updated_at_ms -> BigInt,
        completed_at_ms -> Nullable<BigInt>,
    }
}

diesel::table! {
    items (id) {
        id -> Text,
        run_id -> Text,
        seq -> BigInt,
        kind -> Text,
        role -> Nullable<Text>,
        status -> Text,
        content_json -> Text,
        provider_item_id -> Nullable<Text>,
        call_id -> Nullable<Text>,
        created_at_ms -> BigInt,
    }
}

diesel::table! {
    run_events (id) {
        id -> Text,
        run_id -> Text,
        seq -> BigInt,
        kind -> Text,
        payload_json -> Text,
        persisted_at_ms -> BigInt,
    }
}

diesel::table! {
    tool_executions (id) {
        id -> Text,
        run_id -> Text,
        call_id -> Text,
        tool_name -> Text,
        status -> Text,
        risk -> Text,
        arguments_json -> Text,
        arguments_hash -> Text,
        output_json -> Nullable<Text>,
        error_message -> Nullable<Text>,
        retryable -> Bool,
        prepared_at_ms -> BigInt,
        started_at_ms -> Nullable<BigInt>,
        completed_at_ms -> Nullable<BigInt>,
    }
}

diesel::table! {
    plans (id) {
        id -> Text,
        run_id -> Text,
        revision -> BigInt,
        goal -> Text,
        status -> Text,
        created_at_ms -> BigInt,
    }
}

diesel::table! {
    plan_steps (id) {
        id -> Text,
        plan_id -> Text,
        ordinal -> BigInt,
        title -> Text,
        acceptance -> Text,
        status -> Text,
        attempt -> BigInt,
        created_at_ms -> BigInt,
        updated_at_ms -> BigInt,
    }
}

diesel::table! {
    run_snapshots (run_id) {
        run_id -> Text,
        event_high_water_seq -> BigInt,
        state_json -> Text,
        updated_at_ms -> BigInt,
    }
}

diesel::joinable!(runs -> conversations (conversation_id));
diesel::joinable!(items -> runs (run_id));
diesel::joinable!(run_events -> runs (run_id));
diesel::joinable!(tool_executions -> runs (run_id));
diesel::joinable!(plans -> runs (run_id));
diesel::joinable!(plan_steps -> plans (plan_id));
diesel::joinable!(run_snapshots -> runs (run_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    attachments,
    provider_profiles,
    runs,
    items,
    run_events,
    tool_executions,
    plans,
    plan_steps,
    run_snapshots,
);

diesel::table! {
    attachments (id) {
        id -> Text,
        conversation_id -> Text,
        item_id -> Nullable<Text>,
        file_name -> Text,
        media_type -> Text,
        byte_length -> BigInt,
        content_hash -> Text,
        relative_path -> Text,
        status -> Text,
        created_at_ms -> BigInt,
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
