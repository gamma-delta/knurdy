use kdl::KdlDocument;
use serde::Deserialize;

#[test]
fn to_serde() {
    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct Target {
        an_enum: AnEnum,
        a_kid: Option<Kiddo>,
    }
    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct Kiddo(i32, i32, i32);
    #[derive(Debug, PartialEq, Eq, Deserialize)]
    enum AnEnum {
        Variant1,
        Variant2(String),
    }

    let doc = r#"
    node1 an-enum="Variant1" {
        a-kid 1 2 3
    }

    node-name an-enum=(Variant2)"hello, world"
    "#;

    let node: KdlDocument = doc.parse().unwrap();
    let targets = node
        .nodes()
        .iter()
        .map(|node| knurdy::deserialize_node::<Target>(node))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        targets,
        vec![
            Target {
                an_enum: AnEnum::Variant1,
                a_kid: Some(Kiddo(1, 2, 3))
            },
            Target {
                an_enum: AnEnum::Variant2("hello, world".into()),
                a_kid: None
            }
        ]
    );
}
