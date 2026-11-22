use grace::{Project, import_ascii, open, save, svg};

const DESKTOP_DEFAULTS: &[u8] = include_bytes!("../rust/data/desktop_defaults.agr");

#[test]
fn typed_new_project_accepts_ascii_and_reopens_the_native_project() {
    let mut project = Project::default();
    assert_eq!(project.graphs.len(), 1);
    assert!(project.graphs.contains_key(&0));
    assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
    import_ascii(&mut project, 0, b"# observations\n1, 2\n2, 4\n\n1 3\n2 6\n")
        .expect("load two sets");
    let reopened = open(&save(&project).expect("save AGR")).expect("reopen AGR");
    assert!(
        reopened.diagnostics.is_empty(),
        "{:?}",
        reopened.diagnostics
    );
    assert_eq!(reopened.graphs[&0].sets[&1].rows, [[1., 3.], [2., 6.]]);
    assert_eq!(reopened.graphs[&0].world, project.graphs[&0].world);
    assert_eq!(svg(&project), svg(&reopened));
}

#[test]
fn desktop_defaults_remain_compatible_with_agr_import() {
    let desktop = open(DESKTOP_DEFAULTS).expect("open the desktop defaults");
    assert!(desktop.diagnostics.is_empty(), "{:?}", desktop.diagnostics);
    assert!(desktop.graphs.contains_key(&0));
    assert_eq!(svg(&Project::default()), svg(&desktop));
}

#[test]
fn agr_import_preserves_a_declared_nonzero_graph_id() {
    let project =
        open(b"@version 50125\n@g3 type XY\n@with g3\n").expect("open a project with graph three");
    assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
    assert_eq!(project.graphs.len(), 1);
    assert!(project.graphs.contains_key(&3));
    assert!(!project.graphs.contains_key(&0));
}

#[test]
fn preview_never_serializes_an_unsupported_project() {
    let project = open(b"@version 50125\n@g0 type polar\n@with g0\n").expect("open preview");
    assert_eq!(project.diagnostics[0].line, Some(2));
    assert!(save(&project).is_err());
}

#[test]
fn invalid_ascii_does_not_modify_the_project() {
    let mut project = Project::default();
    let before = svg(&project);
    let diagnostic = import_ascii(&mut project, 0, b"1 2\n3 nope").expect_err("invalid row");
    assert_eq!(diagnostic.line, Some(2));
    assert_eq!(svg(&project), before);
}

#[test]
fn projection_uses_grace_page_coordinates_and_escapes_labels() {
    let project = open(b"@version 50125\n@g0 type XY\n@with g0\n@    title \"A & B < C\"\n@target G0.S0\n@type xy\n0 0\n1 1\n&\n").expect("open project");
    assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
    let svg = svg(&project);
    assert!(svg.contains("viewBox=\"0 0 792 612\""));
    assert!(svg.contains("M91.800,520.200L703.800,91.800"));
    assert!(svg.contains("A &amp; B &lt; C"));
}
