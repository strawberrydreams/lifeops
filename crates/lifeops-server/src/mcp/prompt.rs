use crate::mcp::LifeOpsMcp;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, Role};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IngestArgs {
    /// 엔티티로 분해할 자유 텍스트
    pub text: String,
}

#[rmcp::prompt_router(vis = "pub(crate)")]
impl LifeOpsMcp {
    #[rmcp::prompt(
        name = "ingest",
        description = "자유 텍스트를 기존 타입의 엔티티들로 분해해 저장"
    )]
    async fn ingest(&self, Parameters(args): Parameters<IngestArgs>) -> Vec<PromptMessage> {
        let instructions = format!(
            "다음 텍스트를 LifeOps 엔티티로 분해해 저장하세요.\n\
             절차:\n\
             ① list_types로 존재하는 타입과 필드를 파악한다.\n\
             ② 텍스트를 기존 타입의 엔티티들로 분해한다(맞는 타입이 없으면 만들지 말고 보고).\n\
             ③ query_entities로 중복을 확인한 뒤, 새것이면 create_entity, 기존 것 갱신이면 update_entity.\n\
             ④ 검증 실패 시 반환된 필드별 에러를 보고 값을 고쳐 재시도한다.\n\n\
             텍스트:\n{}",
            args.text
        );
        vec![PromptMessage::new_text(Role::User, instructions)]
    }
}
