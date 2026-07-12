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

`fallback`이 1.5.x 전체의 지원 모드입니다. override는 `DPLYR_PIPE_SYNTAX` 환경 변수(미설정 시 `magrittr`)로 선택된 프로세스 기본 문법을 네이티브 DuckDB AST로 변환하고, 표준 SQL이나 다른 pipe 문법은 기존 parser-extension 경로로 넘깁니다. override 콜백에는 `ClientContext`가 없으므로 세션의 `dplyr_pipe_syntax`만으로 선택한 문법은 직접 override할 수 없습니다.

`strict`는 dplyr 문장을 직접 변환할 수 있지만 DuckDB 1.5.0/1.5.1에서는 override가 처리하지 않는 표준 SQL도 오류가 됩니다. 표준 SQL과 함께 사용할 때는 `fallback`을 사용합니다.
