use sentra_lib::providers::{
    ProviderCandidate, ProviderRegistry, ProviderResolutionStatus, ProviderRouteStatus,
};

#[test]
fn catalog_covers_captured_models_dev_provider_snapshot() {
    let snapshot: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/models-dev-provider-snapshot.json"))
            .expect("models.dev provider snapshot must be valid JSON");
    let providers = snapshot["providers"]
        .as_array()
        .expect("snapshot must contain providers");

    assert_eq!(providers.len(), 159, "snapshot size changed unexpectedly");

    for item in providers {
        let id = item["id"].as_str().expect("provider id must be a string");
        let api = item["api"].as_str();
        let static_api = api.filter(|value| !value.contains('$'));

        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some(id.to_string());
        candidate.configured_base_url = static_api.map(str::to_string);
        let resolved = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(
            resolved.resolution_status,
            ProviderResolutionStatus::Known,
            "models.dev provider {id} is not mapped to a canonical provider"
        );
        if let Some(api) = static_api {
            assert!(
                matches!(
                    resolved.route_status,
                    ProviderRouteStatus::Official | ProviderRouteStatus::Unverified
                ),
                "models.dev provider {id} endpoint {api} is not recognized as a cataloged endpoint: {:?}",
                resolved.route_status
            );
        }
    }
}

#[test]
fn models_dev_only_endpoints_are_not_promoted_to_vendor_verified() {
    let mut candidate = ProviderCandidate::new("opencode");
    candidate.agent_provider_id = Some("stackit".to_string());
    candidate.configured_base_url =
        Some("https://api.openai-compat.model-serving.eu01.onstackit.cloud/v1".to_string());

    let resolved = ProviderRegistry::builtin().resolve(candidate);

    assert_eq!(resolved.route_status, ProviderRouteStatus::Unverified);
    assert_eq!(
        resolved.endpoint_trust,
        Some(sentra_lib::providers::ProviderEndpointTrust::ModelsDev)
    );
}
