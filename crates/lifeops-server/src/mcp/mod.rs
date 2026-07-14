pub mod convert;
pub mod logic;
pub mod params;
pub mod prompt;
pub mod tools;

use crate::state::AppState;
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::ServerHandler;

/// MCP 서버 인스턴스. 세션마다 새로 만들어지며 Arc 기반 AppState를 공유한다.
#[derive(Clone)]
pub struct LifeOpsMcp {
    pub(crate) state: AppState,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl LifeOpsMcp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
#[rmcp::prompt_handler(router = self.prompt_router)]
impl ServerHandler for LifeOpsMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::LATEST)
        .with_instructions(
                "LifeOps는 개인 Life Context 저장소입니다. 데이터는 타입별 엔티티로 저장됩니다.\n\
                 - 무엇이든 넣기 전에 list_types / get_schema로 스키마를 먼저 확인하세요.\n\
                 - 스키마에 맞는 타입이 없으면 새로 만들지 말고 사용자에게 보고하세요(스키마·뷰 변경은 사람 전용).\n\
                 - 중복 저장을 피하려면 create_entity 전에 query_entities로 확인하고, 기존 것이면 update_entity를 쓰세요.\n\
                 - 자유 텍스트를 엔티티로 분해하려면 ingest 프롬프트를 사용하세요."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_state;

    #[tokio::test]
    async fn 도구_9종과_ingest_프롬프트가_등록된다() {
        let (state, _dir) = test_state().await;
        let mcp = LifeOpsMcp::new(state);
        let tool_names = mcp
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();

        for expected in [
            "list_types",
            "get_schema",
            "query_entities",
            "get_entity",
            "create_entity",
            "update_entity",
            "delete_entity",
            "list_pages",
            "run_page",
        ] {
            assert!(
                tool_names.contains(&expected.to_string()),
                "누락된 도구: {expected}"
            );
        }

        let prompt_names = mcp
            .prompt_router
            .list_all()
            .into_iter()
            .map(|prompt| prompt.name.to_string())
            .collect::<Vec<_>>();
        assert!(prompt_names.contains(&"ingest".to_string()));
        assert!(mcp.get_info().instructions.unwrap().contains("list_types"));
    }
}
