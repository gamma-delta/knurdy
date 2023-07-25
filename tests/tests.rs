use kdl::KdlDocument;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Target {
  an_enum: AnEnum,
  a_kid: Option<Kiddo>,
}
#[derive(Debug, PartialEq, Deserialize)]
struct Kiddo(i32, u32, f32);
#[derive(Debug, PartialEq, Deserialize)]
enum AnEnum {
  Variant1,
  Variant2(String),
  Byte(u8),
  Char(char),
}

#[derive(Debug, PartialEq, Deserialize)]
struct Holder {
  foo: u8,
  bar: u8,
  baz: char,
  quxx: char,
}

#[test]
fn to_serde() {
  let doc = r#"
    node1 an-enum="Variant1" {
      a-kid 1 2 3
    }

    node2 an-enum=(Variant2)"goodbye, sunshine" a-kid=null

    node-name an-enum=(Variant2)"hello, world" {
      a-kid null
    }

    amogus an-enum=(Byte)"@" {}
    amogus2 an-enum=(Char)"\u{1F916}"
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
        a_kid: Some(Kiddo(1i32, 2u32, 3.0))
      },
      Target {
        an_enum: AnEnum::Variant2("goodbye, sunshine".into()),
        a_kid: None,
      },
      Target {
        an_enum: AnEnum::Variant2("hello, world".into()),
        a_kid: None,
      },
      Target {
        an_enum: AnEnum::Byte(b'@'),
        a_kid: None
      },
      Target {
        an_enum: AnEnum::Char('\u{1F916}'),
        a_kid: None
      }
    ]
  );
}

#[test]
fn bytes_and_chars_and_nulls() {
  let doc = r#"
    node1 foo=0x41 baz="@" bar="!" quxx=0x42
    node1 quxx="E" foo="?" {
      baz "\u{1F916}"
      bar 0x42
    }

    "#;

  let node: KdlDocument = doc.parse().unwrap();
  let targets = node
    .nodes()
    .iter()
    .map(|node| knurdy::deserialize_node::<Holder>(node))
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(
    targets,
    vec![
      Holder {
        foo: b'A',
        bar: b'!',
        baz: '@',
        quxx: 'B',
      },
      Holder {
        foo: b'?',
        bar: b'B',
        baz: '\u{1F916}',
        quxx: 'E',
      },
    ]
  );
}
