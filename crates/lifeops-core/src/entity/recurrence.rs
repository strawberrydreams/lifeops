use crate::entity::{Entity, EntityStore};
use crate::error::CoreError;
use crate::schema::SchemaSet;
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde_json::Value;

#[derive(Debug)]
pub enum RecurrenceOutcome {
    NotApplicable,
    Spawned(Entity),
    BadRule(String),
}

/// flag가 false→true로 바뀐 update 직후 호출. 규칙이 있으면 다음 회차 엔티티를 생성한다.
pub async fn apply_recurrence(
    store: &EntityStore,
    schemas: &SchemaSet,
    before: &Entity,
    after: &Entity,
    today: NaiveDate,
) -> Result<RecurrenceOutcome, CoreError> {
    let Some(rec) = schemas
        .get(&after.entity_type)
        .and_then(|s| s.behaviors.as_ref())
        .and_then(|b| b.recurrence.as_ref())
    else {
        return Ok(RecurrenceOutcome::NotApplicable);
    };
    let was = before.data.get(&rec.flag).and_then(Value::as_bool).unwrap_or(false);
    let now = after.data.get(&rec.flag).and_then(Value::as_bool).unwrap_or(false);
    if was || !now {
        return Ok(RecurrenceOutcome::NotApplicable);
    }
    let Some(rule_str) = after
        .data
        .get(&rec.rule)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(RecurrenceOutcome::NotApplicable);
    };
    let Some(rule) = parse_rule(rule_str) else {
        return Ok(RecurrenceOutcome::BadRule(rule_str.to_string()));
    };
    let base = after
        .data
        .get(&rec.date)
        .and_then(Value::as_str)
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let next = next_date(&rule, base, today);
    let mut data = after.data.clone();
    data.insert(rec.flag.clone(), Value::Bool(false));
    data.insert(rec.date.clone(), Value::String(next.format("%Y-%m-%d").to_string()));
    let spawned = store.create(schemas, &after.entity_type, data).await?;
    Ok(RecurrenceOutcome::Spawned(spawned))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceRule {
    Daily,
    Weekly(Option<Weekday>),
    Monthly(Option<u32>),
    Yearly(Option<(u32, u32)>),
}

pub fn parse_rule(s: &str) -> Option<RecurrenceRule> {
    let s = s.trim();
    if s == "매일" {
        return Some(RecurrenceRule::Daily);
    }
    if let Some(rest) = s.strip_prefix("매주") {
        let rest = rest.trim();
        if rest.is_empty() {
            return Some(RecurrenceRule::Weekly(None));
        }
        return weekday_ko(rest).map(|w| RecurrenceRule::Weekly(Some(w)));
    }
    if let Some(rest) = s.strip_prefix("매월") {
        let rest = rest.trim();
        if rest.is_empty() {
            return Some(RecurrenceRule::Monthly(None));
        }
        let day: u32 = rest.strip_suffix('일')?.trim().parse().ok()?;
        if !(1..=31).contains(&day) {
            return None;
        }
        return Some(RecurrenceRule::Monthly(Some(day)));
    }
    if let Some(rest) = s.strip_prefix("매년") {
        let rest = rest.trim();
        if rest.is_empty() {
            return Some(RecurrenceRule::Yearly(None));
        }
        let (m, d) = rest.split_once('-')?;
        let m: u32 = m.trim().parse().ok()?;
        let d: u32 = d.trim().parse().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        return Some(RecurrenceRule::Yearly(Some((m, d))));
    }
    None
}

fn weekday_ko(s: &str) -> Option<Weekday> {
    match s {
        "월" => Some(Weekday::Mon),
        "화" => Some(Weekday::Tue),
        "수" => Some(Weekday::Wed),
        "목" => Some(Weekday::Thu),
        "금" => Some(Weekday::Fri),
        "토" => Some(Weekday::Sat),
        "일" => Some(Weekday::Sun),
        _ => None,
    }
}

/// base에서 규칙 단위로 전진, 결과가 오늘 이하이면 오늘을 넘길 때까지 반복.
pub fn next_date(rule: &RecurrenceRule, base: NaiveDate, today: NaiveDate) -> NaiveDate {
    let mut next = step(rule, base);
    while next <= today {
        next = step(rule, next);
    }
    next
}

fn step(rule: &RecurrenceRule, from: NaiveDate) -> NaiveDate {
    match rule {
        RecurrenceRule::Daily => from + Duration::days(1),
        RecurrenceRule::Weekly(None) => from + Duration::days(7),
        RecurrenceRule::Weekly(Some(w)) => {
            let mut d = from + Duration::days(1);
            while d.weekday() != *w {
                d += Duration::days(1);
            }
            d
        }
        RecurrenceRule::Monthly(day) => {
            let target = day.unwrap_or(from.day());
            let (y, m) = if from.month() == 12 {
                (from.year() + 1, 1)
            } else {
                (from.year(), from.month() + 1)
            };
            clamped(y, m, target)
        }
        RecurrenceRule::Yearly(md) => {
            let (m, d) = md.unwrap_or((from.month(), from.day()));
            clamped(from.year() + 1, m, d)
        }
    }
}

/// 해당 월에 없는 일자는 말일로 클램프.
fn clamped(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap_or_else(|| {
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        NaiveDate::from_ymd_opt(ny, nm, 1).expect("유효한 다음 달 1일") - Duration::days(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn 문법_4종_파싱() {
        assert_eq!(parse_rule("매일"), Some(RecurrenceRule::Daily));
        assert_eq!(parse_rule("매주"), Some(RecurrenceRule::Weekly(None)));
        assert_eq!(parse_rule("매주 월"), Some(RecurrenceRule::Weekly(Some(chrono::Weekday::Mon))));
        assert_eq!(parse_rule("매월"), Some(RecurrenceRule::Monthly(None)));
        assert_eq!(parse_rule("매월 15일"), Some(RecurrenceRule::Monthly(Some(15))));
        assert_eq!(parse_rule("매년"), Some(RecurrenceRule::Yearly(None)));
        assert_eq!(parse_rule("매년 3-14"), Some(RecurrenceRule::Yearly(Some((3, 14)))));
        assert_eq!(parse_rule("격주"), None);
        assert_eq!(parse_rule("매월 40일"), None);
        assert_eq!(parse_rule("매년 13-1"), None);
    }

    #[test]
    fn 다음_날짜_기본() {
        let today = d("2026-07-03");
        assert_eq!(next_date(&RecurrenceRule::Daily, d("2026-07-03"), today), d("2026-07-04"));
        assert_eq!(next_date(&RecurrenceRule::Weekly(None), d("2026-07-03"), today), d("2026-07-10"));
        // 2026-07-03은 금요일 → 다음 월요일은 07-06
        assert_eq!(next_date(&RecurrenceRule::Weekly(Some(chrono::Weekday::Mon)), d("2026-07-03"), today), d("2026-07-06"));
        assert_eq!(next_date(&RecurrenceRule::Monthly(Some(15)), d("2026-07-15"), today), d("2026-08-15"));
        assert_eq!(next_date(&RecurrenceRule::Yearly(Some((3, 14))), d("2026-03-14"), today), d("2027-03-14"));
    }

    #[test]
    fn 말일_클램프() {
        let today = d("2026-01-31");
        // 1/31 → 2월엔 31일이 없음 → 2/28
        assert_eq!(next_date(&RecurrenceRule::Monthly(None), d("2026-01-31"), today), d("2026-02-28"));
    }

    #[test]
    fn 밀린_반복은_오늘_이후까지_전진() {
        // 마감일이 한참 과거여도 다음 회차는 오늘 초과
        let today = d("2026-07-03");
        assert_eq!(next_date(&RecurrenceRule::Daily, d("2026-06-01"), today), d("2026-07-04"));
        assert_eq!(next_date(&RecurrenceRule::Weekly(None), d("2026-06-05"), today), d("2026-07-10"));
    }

    use crate::entity::EntityStore;
    use crate::schema::SchemaSet;
    use serde_json::{json, Map, Value};

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn 할일셋() -> SchemaSet {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("할일.yaml"),
            "type: 할일\nbehaviors:\n  recurrence: { flag: 완료, rule: 반복, date: 마감일 }\nfields:\n  내용: { kind: text, required: true }\n  완료: { kind: bool }\n  마감일: { kind: date }\n  반복: { kind: text }\n").unwrap();
        SchemaSet::load_dir(dir.path()).unwrap()
    }

    #[tokio::test]
    async fn 완료_전이시_다음_회차_생성() {
        let s = 할일셋();
        let store = EntityStore::open_in_memory().await.unwrap();
        let e = store.create(&s, "할일", obj(json!({ "내용": "청소", "완료": false, "마감일": "2026-07-03", "반복": "매주" }))).await.unwrap();
        let after = store.update(&s, &e.id, obj(json!({ "완료": true }))).await.unwrap();
        let out = apply_recurrence(&store, &s, &e, &after, d("2026-07-03")).await.unwrap();
        let RecurrenceOutcome::Spawned(next) = out else { panic!("Spawned 기대: {out:?}") };
        assert_eq!(next.data["마감일"], json!("2026-07-10"));
        assert_eq!(next.data["완료"], json!(false));
        assert_eq!(next.data["내용"], json!("청소"));
        assert!(store.get(&next.id).await.unwrap().is_some()); // 실제 저장됨
    }

    #[tokio::test]
    async fn 잘못된_규칙은_badrule_이미_완료였으면_notapplicable() {
        let s = 할일셋();
        let store = EntityStore::open_in_memory().await.unwrap();
        let e = store.create(&s, "할일", obj(json!({ "내용": "x", "완료": false, "반복": "격주" }))).await.unwrap();
        let after = store.update(&s, &e.id, obj(json!({ "완료": true }))).await.unwrap();
        assert!(matches!(apply_recurrence(&store, &s, &e, &after, d("2026-07-03")).await.unwrap(), RecurrenceOutcome::BadRule(r) if r == "격주"));

        // 이미 true → true 는 전이 아님
        let again = store.update(&s, &after.id, obj(json!({ "내용": "y" }))).await.unwrap();
        assert!(matches!(apply_recurrence(&store, &s, &after, &again, d("2026-07-03")).await.unwrap(), RecurrenceOutcome::NotApplicable));
    }
}
