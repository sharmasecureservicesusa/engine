use qovery_engine::cloud_provider::scaleway::application::Zone;
use qovery_engine::cloud_provider::scaleway::kubernetes::node::{Node, NodeType};
use qovery_engine::cloud_provider::scaleway::kubernetes::{Kapsule, KapsuleOptions};
use qovery_engine::cloud_provider::scaleway::Scaleway;
use qovery_engine::cloud_provider::TerraformStateCredentials;
use qovery_engine::container_registry::scaleway_container_registry::ScalewayCR;
use qovery_engine::dns_provider::DnsProvider;
use qovery_engine::engine::Engine;
use qovery_engine::models::{Action, Application, Context, Environment, GitCredentials, Kind, Route, Router};
use qovery_engine::object_storage::scaleway_object_storage::{BucketDeleteStrategy, ScalewayOS};

use crate::cloudflare::dns_provider_cloudflare;
use crate::utilities::{build_platform_local_docker, generate_id, FuncTestsSecrets};

use chrono::Utc;
use std::str::FromStr;
use tracing::error;

pub const ORGANIZATION_ID: &str = "cf8e78e6-159b-45b6-bfb5-2430c9505080";
pub const SCW_TEST_CLUSTER_NAME: &str = "qovery-zb3a2b3b8";
pub const SCW_TEST_CLUSTER_ID: &str = "zb3a2b3b8";
pub const SCW_TEST_ZONE: Zone = Zone::Paris2;
pub const SCW_KUBERNETES_VERSION: &str = "1.18";

pub fn container_registry_scw(context: &Context) -> ScalewayCR {
    let secrets = FuncTestsSecrets::new();
    if secrets.SCALEWAY_ACCESS_KEY.is_none()
        || secrets.SCALEWAY_SECRET_KEY.is_none()
        || secrets.SCALEWAY_DEFAULT_PROJECT_ID.is_none()
    {
        error!("Please check your Vault connectivity (token/address) or SCALEWAY_ACCESS_KEY/SCALEWAY_SECRET_KEY/SCALEWAY_DEFAULT_PROJECT_ID envrionment variables are set");
        std::process::exit(1)
    }
    let random_id = generate_id();
    let scw_secret_key = secrets
        .SCALEWAY_SECRET_KEY
        .expect("SCALEWAY_SECRET_KEY is not set in secrets");
    let scw_default_project_id = secrets
        .SCALEWAY_DEFAULT_PROJECT_ID
        .expect("SCALEWAY_DEFAULT_PROJECT_ID is not set in secrets");

    ScalewayCR::new(
        context.clone(),
        format!("default-registry-qovery-test-{}", random_id.clone()).as_str(),
        format!("default-registry-qovery-test-{}", random_id.clone()).as_str(),
        scw_secret_key.as_str(),
        scw_default_project_id.as_str(),
        SCW_TEST_ZONE,
    )
}

pub fn cloud_provider_scaleway(context: &Context) -> Scaleway {
    let secrets = FuncTestsSecrets::new();

    Scaleway::new(
        context.clone(),
        SCW_TEST_CLUSTER_ID,
        ORGANIZATION_ID,
        SCW_TEST_CLUSTER_NAME,
        secrets
            .SCALEWAY_ACCESS_KEY
            .expect("SCALEWAY_ACCESS_KEY is not set in secrets")
            .as_str(),
        secrets
            .SCALEWAY_SECRET_KEY
            .expect("SCALEWAY_SECRET_KEY is not set in secrets")
            .as_str(),
        TerraformStateCredentials {
            access_key_id: secrets
                .TERRAFORM_AWS_ACCESS_KEY_ID
                .expect("TERRAFORM_AWS_ACCESS_KEY_ID is not set in secrets"),
            secret_access_key: secrets
                .TERRAFORM_AWS_SECRET_ACCESS_KEY
                .expect("TERRAFORM_AWS_SECRET_ACCESS_KEY is not set in secrets"),
            region: "eu-west-3".to_string(),
        },
    )
}

pub fn scw_kubernetes_cluster_options(secrets: FuncTestsSecrets) -> KapsuleOptions {
    KapsuleOptions::new(
        secrets.QOVERY_API_URL.expect("QOVERY_API_URL is not set in secrets"),
        secrets.QOVERY_NATS_URL.expect("QOVERY_NATS_URL is not set in secrets"),
        secrets
            .QOVERY_NATS_USERNAME
            .expect("QOVERY_NATS_USERNAME is not set in secrets"),
        secrets
            .QOVERY_NATS_PASSWORD
            .expect("QOVERY_NATS_PASSWORD is not set in secrets"),
        secrets.QOVERY_SSH_USER.expect("QOVERY_SSH_USER is not set in secrets"),
        "admin".to_string(),
        "qovery".to_string(),
        secrets
            .QOVERY_AGENT_CONTROLLER_TOKEN
            .expect("QOVERY_AGENT_CONTROLLER_TOKEN is not set in secrets"),
        secrets
            .QOVERY_ENGINE_CONTROLLER_TOKEN
            .expect("QOVERY_ENGINE_CONTROLLER_TOKEN is not set in secrets"),
        secrets
            .SCALEWAY_DEFAULT_PROJECT_ID
            .expect("SCALEWAY_DEFAULT_PROJECT_ID is not set in secrets"),
        secrets
            .SCALEWAY_ACCESS_KEY
            .expect("SCALEWAY_ACCESS_KEY is not set in secrets"),
        secrets
            .SCALEWAY_SECRET_KEY
            .expect("SCALEWAY_SECRET_KEY is not set in secrets"),
        secrets
            .LETS_ENCRYPT_EMAIL_REPORT
            .expect("LETS_ENCRYPT_EMAIL_REPORT is not set in secrets"),
    )
}

pub fn scw_object_storage(context: Context, region: Zone) -> ScalewayOS {
    let secrets = FuncTestsSecrets::new();
    let random_id = generate_id();

    ScalewayOS::new(
        context,
        format!("qovery-test-object-storage-{}", random_id.clone()),
        format!("Qovery Test Object-Storage {}", random_id),
        secrets
            .SCALEWAY_ACCESS_KEY
            .expect("SCALEWAY_ACCESS_KEY is not set in secrets"),
        secrets
            .SCALEWAY_SECRET_KEY
            .expect("SCALEWAY_SECRET_KEY is not set in secrets"),
        region,
        BucketDeleteStrategy::Empty, // do not delete bucket due to deletion 24h delay
        false,
    )
}

pub fn scw_kubernetes_nodes() -> Vec<Node> {
    // Note: Dev1M is a bit too small to handle engine + local docker, hence using Dev1L
    scw_kubernetes_custom_nodes(10, NodeType::Dev1L)
}

pub fn scw_kubernetes_custom_nodes(count: usize, node_type: NodeType) -> Vec<Node> {
    vec![Node::new(node_type); count]
}

pub fn docker_scw_cr_engine(context: &Context) -> Engine {
    // use Scaleway CR
    let container_registry = Box::new(container_registry_scw(context));

    // use LocalDocker
    let build_platform = Box::new(build_platform_local_docker(context));

    // use Scaleway
    let cloud_provider = Box::new(cloud_provider_scaleway(context));

    let dns_provider = Box::new(dns_provider_cloudflare(context));

    Engine::new(
        context.clone(),
        build_platform,
        container_registry,
        cloud_provider,
        dns_provider,
    )
}

pub fn scw_kubernetes_kapsule<'a>(
    context: &Context,
    cloud_provider: &'a Scaleway,
    dns_provider: &'a dyn DnsProvider,
    nodes: Vec<Node>,
) -> Kapsule<'a> {
    let secrets = FuncTestsSecrets::new();
    Kapsule::<'a>::new(
        context.clone(),
        SCW_TEST_CLUSTER_ID.to_string(),
        SCW_TEST_CLUSTER_NAME.to_string(),
        SCW_KUBERNETES_VERSION.to_string(),
        Zone::from_str(
            secrets
                .clone()
                .SCALEWAY_DEFAULT_REGION
                .expect("SCALEWAY_DEFAULT_REGION is not set in secrets")
                .as_str(),
        )
        .unwrap(),
        cloud_provider,
        dns_provider,
        nodes,
        scw_kubernetes_cluster_options(secrets),
    )
}

// TODO(benjaminch): To be refactored, move it to common test utilities
pub fn working_minimal_environment(context: &Context, secrets: FuncTestsSecrets) -> Environment {
    let suffix = generate_id();
    Environment {
        execution_id: context.execution_id().to_string(),
        id: generate_id(),
        kind: Kind::Development,
        owner_id: generate_id(),
        project_id: secrets
            .SCALEWAY_DEFAULT_PROJECT_ID
            .expect("SCALEWAY_DEFAULT_PROJECT_ID is not set in secrets"),
        organization_id: ORGANIZATION_ID.to_string(),
        action: Action::Create,
        applications: vec![Application {
            id: generate_id(),
            name: format!("{}-{}", "simple-app".to_string(), &suffix),
            git_url: "https://github.com/Qovery/engine-testing.git".to_string(),
            commit_id: "fc575a2f3be0b9100492c8a463bf18134a8698a5".to_string(),
            dockerfile_path: Some("Dockerfile".to_string()),
            root_path: String::from("/"),
            action: Action::Create,
            git_credentials: Some(GitCredentials {
                login: "x-access-token".to_string(),
                access_token: "xxx".to_string(),
                expired_at: Utc::now(),
            }),
            storage: vec![],
            environment_variables: vec![],
            branch: "basic-app-deploy".to_string(),
            private_port: Some(80),
            total_cpus: "100m".to_string(),
            total_ram_in_mib: 256,
            total_instances: 2,
            cpu_burst: "100m".to_string(),
            start_timeout_in_seconds: 60,
        }],
        routers: vec![Router {
            id: generate_id(),
            name: "main".to_string(),
            action: Action::Create,
            default_domain: format!(
                "{}.{}",
                generate_id(),
                secrets
                    .DEFAULT_TEST_DOMAIN
                    .expect("DEFAULT_TEST_DOMAIN is not set in secrets")
            ),
            public_port: 443,
            custom_domains: vec![],
            routes: vec![Route {
                path: "/".to_string(),
                application_name: format!("{}-{}", "simple-app".to_string(), &suffix),
            }],
        }],
        databases: vec![],
        external_services: vec![],
        clone_from_environment_id: None,
    }
}

// TODO(benjaminch): To be refactored, move it to common test utilities
pub fn non_working_environment(context: &Context, secrets: FuncTestsSecrets) -> Environment {
    let mut environment = working_minimal_environment(context, secrets);

    environment.applications = environment
        .applications
        .into_iter()
        .map(|mut app| {
            app.git_url = "https://github.com/Qovery/engine-testing.git".to_string();
            app.branch = "bugged-image".to_string();
            app.commit_id = "c2b2d7b5d96832732df25fe992721f53842b5eac".to_string();
            app
        })
        .collect::<Vec<_>>();

    environment
}
