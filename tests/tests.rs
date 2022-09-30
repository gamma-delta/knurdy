use ahash::AHashMap;
use knurdy::*;
use serde::{de::IntoDeserializer, Deserialize};

#[test]
fn knuffel_to_node() {
    let doc = r#"
foo "arg1" 2 true k="v" k2="v2" {
    kiddo1 "hi"
    kiddo2 (annotation)"hewwo"
}
    "#;
    let nodes: Vec<KdlNode> = knuffel::parse("test", doc).unwrap();
    assert_eq!(nodes.len(), 1);
    let node = &nodes[0];

    assert_eq!(
        node,
        &KdlNode {
            name: "foo".into(),
            arguments: vec![
                KdlAnnotatedLiteral::new(None, KdlLiteral::String("arg1".into())),
                KdlAnnotatedLiteral::new(None, KdlLiteral::Int(2)),
                KdlAnnotatedLiteral::new(None, KdlLiteral::Bool(true)),
            ],
            properties: [
                (
                    "k".into(),
                    KdlAnnotatedLiteral::new(None, KdlLiteral::String("v".into()))
                ),
                (
                    "k2".into(),
                    KdlAnnotatedLiteral::new(None, KdlLiteral::String("v2".into()))
                ),
            ]
            .into_iter()
            .collect(),
            children: Some(vec![
                KdlNode {
                    name: "kiddo1".into(),
                    arguments: vec![KdlAnnotatedLiteral::new(
                        None,
                        KdlLiteral::String("hi".into())
                    )],
                    properties: AHashMap::default(),
                    children: None,
                },
                KdlNode {
                    name: "kiddo2".into(),
                    arguments: vec![KdlAnnotatedLiteral::new(
                        Some("annotation".into()),
                        KdlLiteral::String("hewwo".into())
                    )],
                    properties: AHashMap::default(),
                    children: None,
                }
            ])
        }
    )
}

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

    let nodes: Vec<KdlNode> = knuffel::parse("test", doc).unwrap();
    let targets = nodes
        .iter()
        .map(|node| Target::deserialize(node.into_deserializer()))
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
