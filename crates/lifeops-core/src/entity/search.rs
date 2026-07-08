use crate::entity::Entity;
use crate::schema::{FieldKind, ResolvedSchema};
use serde_json::Value;

/// richtext(HTML) → 검색용 평문. 태그 제거 + 흔한 엔티티 디코드 + 공백 축약.
pub fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // &amp;는 마지막에 디코드(이중 디코드 방지)
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 엔티티의 검색 대상 텍스트를 (필드명, 텍스트)로 추출한다.
pub fn searchable_fields(schema: &ResolvedSchema, entity: &Entity) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (fname, fdef) in &schema.fields {
        let Some(value) = entity.data.get(fname) else {
            continue;
        };
        for text in field_texts(&fdef.kind, value) {
            if !text.is_empty() {
                out.push((fname.clone(), text));
            }
        }
    }
    out.push(("타입".to_string(), entity.entity_type.clone()));
    out
}

fn field_texts(kind: &FieldKind, value: &Value) -> Vec<String> {
    match kind {
        FieldKind::Text | FieldKind::Enum => {
            value.as_str().map(|s| vec![s.to_string()]).unwrap_or_default()
        }
        FieldKind::RichText => value.as_str().map(|s| vec![strip_html(s)]).unwrap_or_default(),
        FieldKind::List(inner) if matches!(**inner, FieldKind::Text | FieldKind::Enum) => value
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
        FieldKind::List(inner) if **inner == FieldKind::RichText => value
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(strip_html)).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_태그_엔티티_공백을_정리한다() {
        assert_eq!(strip_html("<p>안녕 <b>미쿠</b></p>"), "안녕 미쿠");
        assert_eq!(strip_html("a &amp; b &lt;tag&gt;"), "a & b <tag>");
        assert_eq!(strip_html("  여러   공백\n줄바꿈  "), "여러 공백 줄바꿈");
    }

    use crate::schema::SchemaSet;
    use serde_json::json;

    fn note_schema_entity() -> (ResolvedSchema, Entity) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("노트.yaml"),
            "type: 노트\nfields:\n  제목: { kind: text }\n  본문: { kind: richtext }\n  태그: { kind: enum, options: [일기, 회고] }\n  점수: { kind: number }\n",
        )
        .unwrap();
        let set = SchemaSet::load_dir(dir.path()).unwrap();
        let schema = set.get("노트").unwrap().clone();
        let entity = Entity {
            id: "n1".into(),
            entity_type: "노트".into(),
            data: json!({ "제목": "여름 회고", "본문": "<p>세이코를 <b>팔</b>았다</p>", "태그": "회고", "점수": 3 })
                .as_object()
                .unwrap()
                .clone(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        (schema, entity)
    }

    #[test]
    fn searchable_fields_텍스트_richtext_enum_타입명포함_숫자와필드명제외() {
        let (schema, entity) = note_schema_entity();
        let fields = searchable_fields(&schema, &entity);
        let texts: Vec<&str> = fields.iter().map(|(_, t)| t.as_str()).collect();
        assert!(texts.contains(&"여름 회고"));                     // text
        assert!(texts.iter().any(|t| t.contains("세이코를 팔았다"))); // richtext 태그 제거
        assert!(texts.contains(&"회고"));                          // enum 값
        assert!(texts.contains(&"노트"));                          // 타입명
        assert!(!texts.iter().any(|t| t.contains('3')));           // number 제외
        assert!(!texts.contains(&"제목"));                         // 필드명 자체 제외
    }
}
