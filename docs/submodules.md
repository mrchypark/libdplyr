# Submodules

이 프로젝트는 빌드/테스트 재현성을 위해 Git submodule을 “커밋 단위로 고정(pinned)”해서 사용합니다.

## 정책

- submodule 업데이트는 **명시적인 PR**로만 수행합니다.
- submodule 커밋이 바뀌면 릴리즈 노트(예: `RELEASE_NOTES_*.md`)에 변경 사실을 적습니다.
- `git submodule update --remote`는 의도치 않은 업데이트를 유발할 수 있으므로 사용을 피합니다.
- Makefile에 포함된 upstream 타겟(`update`, `pull`)은 `--remote`를 사용할 수 있으니, 대신 `make submodules` / `make submodules-init`을 사용합니다.

## 현재 고정 상태

- `duckdb`: `b8a06e4a22672e254cd0baa68a3dbed2eb51c56e` (`v1.4.0`)
- `extension-ci-tools`: `00a4d1a464a0ebd30c7427396c0271105592d63b`
