# Submodules

이 프로젝트는 빌드/테스트 재현성을 위해 Git submodule을 “커밋 단위로 고정(pinned)”해서 사용합니다.

## 정책

- submodule 업데이트는 **명시적인 PR**로만 수행합니다.
- submodule 커밋이 바뀌면 릴리즈 노트(예: `RELEASE_NOTES_*.md`)에 변경 사실을 적습니다.
- `git submodule update --remote`는 의도치 않은 업데이트를 유발할 수 있으므로 사용을 피합니다.
- Makefile에 포함된 upstream 타겟(`update`, `pull`)은 `--remote`를 사용할 수 있으니, 대신 `make submodules` / `make submodules-init`을 사용합니다.

## 현재 고정 상태

- `duckdb`: `08e34c447bae34eaee3723cac61f2878b6bdf787` (`v1.5.4`)
- `extension-ci-tools`: `72e76e99cd7fee45a99739cd118ec2db64e034ec` (`v1.5-variegata`)

## Compatibility Matrix

- 최소 지원 DuckDB 버전: `v1.5.0`
- 주 지원 DuckDB 버전: `v1.5.4`
- 최소 호환성 검사 버전: `v1.5.0`
- CI는 현재 `v1.5.4`를 멀티플랫폼 주 검증선으로, `v1.5.0`을 Linux/macOS 호환성 검증선으로 사용합니다.
- 릴리스 바이너리는 현재 `v1.5.0`과 `v1.5.4`를 대상으로 생성합니다.
- 호환성 매트릭스와 로컬 개발용 submodule 핀은 별개로 관리하되, 현재 DuckDB 핀은 주 지원 버전 `v1.5.4`와 일치합니다.

## Parser override

DuckDB 1.5.x에서는 다음 설정으로 dplyr parser override를 명시적으로 활성화할 수 있습니다.

```sql
SET allow_parser_override_extension = 'fallback';
```

`fallback`이 1.5.x 전체의 지원 모드입니다. override는 DB-global `dplyr_pipe_syntax` 설정을 따르며, 설정이 없으면 `DPLYR_PIPE_SYNTAX` 환경 변수(미설정 시 `magrittr`)를 사용합니다. `SET GLOBAL dplyr_pipe_syntax = 'native'`처럼 설정합니다. DuckDB parser override 콜백에는 `ClientContext`가 없어서 session-local 값은 암시적 pipeline에 영향을 주지 않습니다. 연결별 문법이 필요하면 `dplyr(query, mode)`의 명시적 `mode`를 사용합니다. 이 제약으로 parser override가 기존 parser-extension 실행 경로로 우회하지 않으며, 생성 SQL은 호출자의 임시 테이블과 미커밋 트랜잭션 상태를 그대로 사용합니다.

`strict`는 DuckDB 1.5.x 전체에서 dplyr 문장과 표준 SQL을 직접 네이티브 AST로 변환합니다. DuckDB 1.5.0/1.5.1은 override의 `NotHandled`를 즉시 오류로 처리하므로, 전체 1.5.x에 동일한 fallback 동작이 필요하거나 다른 parser override extension과 함께 사용할 때는 `fallback`을 사용합니다.
