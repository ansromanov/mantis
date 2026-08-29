use super::*;

#[test]
fn non_manifest_yaml_produces_no_facts() {
    let src = "foo: bar\nlist:\n  - a\n  - b\n";
    assert_eq!(format_status_facts(src), "");
}

#[test]
fn kind_without_api_version_is_not_a_manifest() {
    let src = "kind: Pod\nmetadata:\n  name: web\n";
    assert_eq!(format_status_facts(src), "");
}

#[test]
fn api_version_without_kind_is_not_a_manifest() {
    let src = "apiVersion: v1\nmetadata:\n  name: web\n";
    assert_eq!(format_status_facts(src), "");
}

#[test]
fn single_resource_reports_identity_with_namespace() {
    let src = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx
  namespace: web
  labels:
    app: nginx
";
    assert_eq!(format_status_facts(src), "Deployment/nginx (web)");
}

#[test]
fn single_resource_without_namespace_omits_parens() {
    let src = "\
apiVersion: v1
kind: Namespace
metadata:
  name: web
";
    assert_eq!(format_status_facts(src), "Namespace/web");
}

#[test]
fn resource_without_name_falls_back_to_question_mark() {
    let src = "apiVersion: v1\nkind: ConfigMap\n";
    assert_eq!(format_status_facts(src), "ConfigMap/?");
}

#[test]
fn multi_doc_reports_first_identity_and_per_kind_counts() {
    let src = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx
  namespace: default
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: worker
  namespace: default
---
apiVersion: v1
kind: Service
metadata:
  name: nginx-svc
  namespace: default
---
apiVersion: v1
kind: Service
metadata:
  name: worker-svc
  namespace: default
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: app-config
  namespace: default
";
    assert_eq!(
        format_status_facts(src),
        "Deployment/nginx (default) \u{b7} 2 Deployments \u{b7} 2 Services \u{b7} 1 ConfigMap"
    );
}

#[test]
fn non_manifest_documents_in_a_multi_doc_file_are_skipped() {
    let src = "\
apiVersion: v1
kind: Pod
metadata:
  name: web
---
just: some_config
---
apiVersion: v1
kind: Pod
metadata:
  name: web2
";
    assert_eq!(format_status_facts(src), "Pod/web \u{b7} 2 Pods");
}

#[test]
fn nested_metadata_key_does_not_shadow_name_or_namespace() {
    // `labels.name` sits deeper than metadata's real children (name/namespace)
    // and must not be picked up as the resource's name.
    let src = "\
apiVersion: v1
kind: Pod
metadata:
  labels:
    name: should-not-be-picked
  name: real-name
  namespace: ns1
";
    assert_eq!(format_status_facts(src), "Pod/real-name (ns1)");
}

#[test]
fn quoted_scalars_are_unquoted() {
    let src = "\
apiVersion: v1
kind: \"Pod\"
metadata:
  name: 'web'
";
    assert_eq!(format_status_facts(src), "Pod/web");
}

#[test]
fn pluralize_common_kinds() {
    assert_eq!(pluralize("Deployment"), "Deployments");
    assert_eq!(pluralize("Service"), "Services");
    assert_eq!(pluralize("ConfigMap"), "ConfigMaps");
    assert_eq!(pluralize("Ingress"), "Ingresses");
    assert_eq!(pluralize("NetworkPolicy"), "NetworkPolicies");
    assert_eq!(pluralize("StorageClass"), "StorageClasses");
    assert_eq!(pluralize("Pod"), "Pods");
}

#[test]
fn comment_and_blank_lines_are_ignored() {
    let src = "\
# a comment
apiVersion: v1

kind: Pod
metadata:
  # another comment
  name: web
";
    assert_eq!(format_status_facts(src), "Pod/web");
}
