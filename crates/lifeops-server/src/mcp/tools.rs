use crate::mcp::params::{
    CreateParams, DeleteParams, GetEntityParams, GetSchemaParams, QueryParams, RunPageParams,
    UpdateParams,
};
use crate::mcp::LifeOpsMcp;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};

impl LifeOpsMcp {
    fn ok(value: serde_json::Value) -> CallToolResult {
        let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    fn fail(message: String) -> CallToolResult {
        CallToolResult::error(vec![ContentBlock::text(message)])
    }

    fn from_result(result: Result<serde_json::Value, String>) -> CallToolResult {
        match result {
            Ok(value) => Self::ok(value),
            Err(message) => Self::fail(message),
        }
    }
}

#[rmcp::tool_router(vis = "pub(crate)")]
impl LifeOpsMcp {
    #[rmcp::tool(description = "전체 타입 요약(이름·카테고리·필드·상속)")]
    async fn list_types(&self) -> CallToolResult {
        Self::from_result(self.do_list_types().await)
    }

    #[rmcp::tool(description = "한 타입의 해석된 스키마(필드 kind·required·options·target)")]
    async fn get_schema(&self, Parameters(params): Parameters<GetSchemaParams>) -> CallToolResult {
        Self::from_result(self.do_get_schema(&params.type_name).await)
    }

    #[rmcp::tool(description = "타입+필터+정렬+limit 조회(뷰 엔진과 같은 의미론, $today±Nd 지원)")]
    async fn query_entities(&self, Parameters(params): Parameters<QueryParams>) -> CallToolResult {
        Self::from_result(self.do_query_entities(params).await)
    }

    #[rmcp::tool(description = "id로 단건 조회 + 역링크")]
    async fn get_entity(&self, Parameters(params): Parameters<GetEntityParams>) -> CallToolResult {
        Self::from_result(self.do_get_entity(&params.id).await)
    }

    #[rmcp::tool(description = "엔티티 생성(스키마 검증 통과 필수, 실패 시 필드별 오류)")]
    async fn create_entity(&self, Parameters(params): Parameters<CreateParams>) -> CallToolResult {
        Self::from_result(self.do_create_entity(&params.type_name, params.data).await)
    }

    #[rmcp::tool(description = "부분 수정(PATCH). 반복 할일 완료 시 다음 회차를 spawned로 반환")]
    async fn update_entity(&self, Parameters(params): Parameters<UpdateParams>) -> CallToolResult {
        Self::from_result(self.do_update_entity(&params.id, params.patch).await)
    }

    #[rmcp::tool(description = "엔티티 삭제(참조 존재 시 거부하고 참조 목록 반환)")]
    async fn delete_entity(&self, Parameters(params): Parameters<DeleteParams>) -> CallToolResult {
        Self::from_result(self.do_delete_entity(&params.id).await)
    }

    #[rmcp::tool(description = "페이지(뷰) 목록")]
    async fn list_pages(&self) -> CallToolResult {
        Self::from_result(self.do_list_pages().await)
    }

    #[rmcp::tool(description = "페이지 실행 결과(블록별 엔티티·집계)")]
    async fn run_page(&self, Parameters(params): Parameters<RunPageParams>) -> CallToolResult {
        Self::from_result(self.do_run_page(&params.name).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 도구_실패는_is_error와_텍스트로_전달된다() {
        let result = LifeOpsMcp::from_result(Err("필드 '가격': money 객체 필요".to_string()));

        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content.len(), 1);
        let text = result.content[0].as_text().expect("텍스트 콘텐츠");
        assert!(text.text.contains("가격"));
        assert!(text.text.contains("money"));
    }
}
