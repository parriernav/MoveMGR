# 데이터·명령 계약

이 문서의 TypeScript는 구현할 데이터 형식을 설명하는 명세다. Rust serde 모델이 런타임 검증의 기준이며 태그가 있는 enum으로 각 모드의 필드를 제한한다. 선택하지 않은 모드의 숨겨진 값이 실행에 영향을 주면 안 된다.

## 1. 설정

```ts
type Extractor =
  | { kind: 'whole' }
  | { kind: 'firstChars'; count: number } // 정수 1..255
  | { kind: 'beforeDelimiter'; delimiter: string;
      missing: 'skip' | 'useWhole' }; // 구분자 1..32 graphemes

type NameFilter = {
  op: 'startsWith' | 'contains' | 'endsWith' | 'equals';
  value: string; // 빈 문자열 불가, 여러 필터는 AND
};

type ExtensionFilter =
  | { mode: 'all' }
  | { mode: 'only'; values: string[]; includeExtensionless: boolean };

type Comparison =
  | { kind: 'equals' }
  | { kind: 'startsWith' }
  | { kind: 'prefixEqual'; count: number };

type Destination =
  | { mode: 'root' }
  | { mode: 'fixedSubfolder'; relativePath: string; createIfMissing: boolean }
  | { mode: 'matchSubfolder'; searchBase: string; folderExtractor: Extractor;
      comparison: Comparison; noMatch: 'skip' | 'createKeyFolder' }
  | { mode: 'keySubfolder'; parentRelativePath: string; createIfMissing: boolean };

type Project = {
  id: string; // UUID
  name: string;
  checked: boolean;
  source: {
    root: string;
    recursive: boolean;
    includeHidden: boolean;
    extensions: ExtensionFilter;
    nameFilters: NameFilter[];
  };
  key: { extractor: Extractor; trim: boolean };
  comparisonOptions: { ignoreCase: boolean; normalization: 'NFC' };
  target: { root: string; destination: Destination };
  conflict: 'skip' | 'renameWithNumber';
};

type RuleTag =
  | { id: string; name: string; kind: 'source'; rules: Pick<Project['source'], 'recursive' | 'includeHidden' | 'extensions' | 'nameFilters'> }
  | { id: string; name: string; kind: 'target'; rules: { key: Project['key']; comparisonOptions: Project['comparisonOptions']; destination: Destination; conflict: Project['conflict'] } };

type PortableSettings = {
  format: 'movemgr-settings';
  schemaVersion: 1;
  exportedByAppVersion: string;
  exportedAt: string; // UTC ISO-8601
  preferences: {
    theme: 'system' | 'light' | 'dark';
    language: 'ko';
    previewBeforeRun: boolean;
  };
  projects: Project[]; // 배열 순서 = 화면 순서 = 실행 순서
  ruleTags: RuleTag[]; // 소스·타겟 조건 태그, 이전 설정에서 누락되면 빈 배열
};

type LocalState = {
  schemaVersion: 1;
  revision: number; // 매 영속 변경마다 단조 증가, 가져오기 시 재부여
  preferences: PortableSettings['preferences'];
  projects: Project[];
  ruleTags: RuleTag[];
  window: { width: number; height: number; maximized: boolean };
};
```

이름 필터의 ‘앞 N글자가 특정 문자열’은 문자열 길이 N의 `startsWith`로 표현한다. 분류값 추출의 firstChars.count는 별도다. 파일 선별과 분류값의 역할을 혼동하지 않는다.

comparisonOptions는 파일명 필터 및 파일/폴더 값 비교에 함께 적용한다. 확장자 비교는 항상 ASCII case-insensitive다. `key.trim`은 파일 분류값과 폴더 비교값 모두에 적용한다. 파일명 필터에서는 사용자가 입력한 공백을 유지한다.

빈 searchBase/parentRelativePath는 타겟 루트다. relativePath는 `/`로 구분한 논리적 상대 경로로 저장하고 Rust에서 OS 경로로 변환한다. 루트 경로는 선택한 OS의 절대 경로를 보존한다. 상대 경로의 절대 경로·드라이브 접두어·UNC·빈 중간 segment·`.`·`..`는 허용하지 않는다.

keySubfolder는 이미 동일 이름의 폴더가 있으면 재사용하며 createIfMissing=false이고 없으면 건너뛴다. fixedSubfolder가 없고 생성도 꺼져 있으면 프로젝트 실행 전 검증에서 차단한다. matchSubfolder의 검색 기준 폴더는 반드시 존재해야 하며 noMatch 생성은 그 안에서만 허용한다.

키를 사용하지 않는 root/fixedSubfolder에서는 저장된 key 설정을 무시하며 분류값 추출 실패로 파일을 제외하지 않는다. 매칭/키 폴더 모드에서만 키 추출을 수행한다.

## 2. 검증·저장 규약

- 프로젝트명 1..80 graphemes, 시작·끝 공백 제거. 같은 이름은 허용하되 UUID로 식별한다.
- 저장 가능한 규칙 유효성(필수값, count, enum, 상대 경로)과 실행 가능한 경로 상태(존재, 접근권한)를 분리한다.
- 경로가 연결되지 않아도 프로젝트를 저장할 수 있다. runtime의 `readiness`에 `pathRequired`/`unavailable`을 표시한다. 가져온 데이터에 readiness를 신뢰하지 않는다.
- 입력·가져오기 문자열에 NUL 금지. 파일명에서 얻은 분류값을 경로에 사용할 때는 플랫폼 공통 파일명 검증 추가.
- JSON 모르는 enum 및 미래 schemaVersion은 거부한다. 첫 버전에는 unknown field도 오류로 보고 적용 전 필드 위치를 표시한다. 자동 마이그레이션은 알려진 이전 버전에만 적용한다.
- 설정 변경 명령은 expectedRevision을 받고 충돌하면 저장을 거부해 최신 상태를 다시 읽도록 한다. Rust backend가 한 writer를 갖는다.
- 저장 결과는 전체 최신 LocalState를 반환한다. UI는 저장 실패 시 기존 영속값과 편집 초안을 구분한다.
- 앱 종료 전 pending save를 flush한다. 비정상 강제 종료 직전 디스크에 쓰이지 않은 상태까지 보장하지는 않는다.
- 실행 기록은 설정 JSON에 넣지 않는다. 설정을 교체해도 기존 실행 기록을 지우지 않는다.

## 3. 계획과 실행 상태

```ts
type PlanDecision = 'move' | 'skip' | 'blocked';
type PlannedItem = {
  itemId: string;
  projectId: string;
  sourcePath: string;
  extractedKey: string | null;
  proposedTargetPath: string | null;
  decision: PlanDecision;
  reasonCode: string | null;
  reasonArgs: Record<string, string | number>;
  sizeBytes: string; // JS 안전 정수 범위를 넘는 u64를 문자열로 전달
};

type RunState =
  | 'planning' | 'awaitingReview' | 'running' | 'cancelling'
  | 'completed' | 'completedWithIssues' | 'cancelled' | 'failed';

type ItemState =
  | 'planned' | 'copying' | 'verified' | 'targetCommitted'
  | 'moved' | 'skipped' | 'failed' | 'cancelled'
  | 'sourceRetained' | 'needsReview';
```

plan에는 별도로 backend 전용 source ID·mtime·volume identity·설정 revision·설정 hash·요약을 저장한다. 프런트엔드가 보내는 targetPath를 실행 근거로 쓰지 않는다. 최종 목적지는 backend plan에서만 얻는다. draft plan은 `executable=false`다.

항목에는 발견 당시 순서를 부여한다. 프로젝트 순서, 원본의 루트 기준 상대 경로 순으로 결정하며 같은 비교값이면 원본 경로 바이트를 tie-break로 사용한다. 명확한 충돌 우선순위와 반복 가능한 미리보기를 위한 규칙이다.

처리 중 숫자: `발견 수`, `대상 수`, `처리 완료 수`, `이동`, `건너뜀`, `실패`, `원본 남음`, `확인 필요`. 실패/원본 남음이 있으면 전체 성공으로 표시하지 않는다. 0개 대상은 정상 완료(이동 대상 없음)다. 취소 후 미시작 항목은 cancelled다.

조건에서 제외된 파일은 ‘조건 제외’ 개수로 요약하고 전체 파일별 저널은 만들지 않아도 된다. 필터를 통과했지만 키 추출·타겟 매칭에 실패한 파일은 계획표에 원인과 함께 남긴다.

## 4. IPC 표면

아래 이름은 구현 시 유지한다. Rust 오류는 `{code, messageKey, args, retryable}`로 구조화하고 UI에서 한국어로 렌더링한다.

| Command | 입력 | 결과 / 동작 |
| --- | --- | --- |
| get_app_info | 없음 | 앱 버전, digest, OS, 지원 기능 |
| load_state | 없음 | LocalState, 복구 필요 여부 |
| save_project | project, expectedRevision | 검증·저장 후 최신 state |
| delete_project | id, expectedRevision | 설정만 삭제, 파일 삭제 없음 |
| duplicate_project | id, expectedRevision | 새 UUID·복제 이름·checked=false |
| set_selection | checkedIds, expectedRevision | 원자적으로 전체 체크 상태 반영 |
| reorder_projects | orderedIds, expectedRevision | ID 집합 검증 후 저장 |
| save_preferences | preferences, expectedRevision | 저장된 state |
| evaluate_example | draftRule, sampleName, sampleFolders | I/O 없는 동일 엔진 평가 |
| create_plan | projectIds 또는 draftProject | jobId 즉시 반환, 비동기 plan 생성 |
| get_plan_page | planId, cursor, limit, decisionFilter | 계획 행·요약·nextCursor |
| cancel_plan | jobId | 읽기 전용 스캔 취소 |
| start_run | planId, allowEligibleOnly, requestId | runId, 중복 requestId는 같은 결과 |
| cancel_run | runId | cancelling, 완료 이벤트는 별도 |
| get_run_snapshot | runId, afterSequence? | 현재 상태·카운터·최근 이벤트 |
| list_history | cursor, limit | 실행 요약 목록 |
| get_history_page | runId, cursor, limit | 항목 결과·실제 최종 경로 |
| inspect_recovery | runId | 디스크와 저널 대조, 자동 삭제 없음 |
| preview_import | 읽을 파일 경로 | 검증된 importToken·차이·재연결 목록 |
| apply_import | importToken, mode, pathMappings, expectedRevision | 일괄 저장, 만료 token 거부 |
| export_settings | 저장할 파일 경로 | 저장된 snapshot 내보내기 |

폴더/파일 선택기는 공식 dialog 플러그인으로 연결한다. 외부 경로를 받는 import/export 명령에서도 입력 검증과 사용자 선택 경로 범위를 확인한다. 소스 경로가 export 대상과 충돌하는 경우 저장 전에 확인을 요구한다. 일반 파일 이동 권한을 프런트엔드에 통째로 노출하지 않는다.

events: `plan-progress`, `plan-ready`, `plan-failed`, `run-progress`, `run-finished`, `state-save-failed`.

모든 비동기 이벤트는 jobId 또는 runId, monotonic sequence를 포함한다. UI는 오래된 이벤트를 무시하고 재연결 시 snapshot으로 복구한다. 이벤트만을 영속 진실로 사용하지 않는다. 목록 조회 limit은 기본 100, 최대 500이다.

## 5. 오류·사용자 문구

| code | 사용자에게 보이는 의미 | 기본 처리 |
| --- | --- | --- |
| INVALID_CONFIG | 입력한 조건을 확인해주세요 | 저장/실행 거부 |
| PATH_UNAVAILABLE | 폴더를 찾거나 열 수 없습니다 | 프로젝트 차단 |
| ROOT_OVERLAP | 소스와 타겟의 범위가 겹칩니다 | 실행 묶음 차단 |
| SOURCE_SHARED | 같은 소스가 여러 프로젝트에 포함됩니다 | 실행 묶음 차단 |
| KEY_TOO_SHORT | 이름이 지정한 글자 수보다 짧습니다 | 파일 건너뜀 |
| DELIMITER_MISSING | 이름에 구분자가 없습니다 | skip 설정일 때 건너뜀 |
| EMPTY_KEY | 분류값이 비어 있습니다 | 파일 건너뜀 |
| NO_TARGET_MATCH | 일치하는 하위 폴더가 없습니다 | 설정에 따라 건너뜀/생성 |
| AMBIGUOUS_TARGET | 일치하는 폴더가 여러 개입니다 | 보류, 후보 표시 |
| INVALID_TARGET_NAME | 분류값을 폴더 이름으로 사용할 수 없습니다 | 파일 보류 |
| TARGET_EXISTS | 같은 이름의 파일이 있습니다 | 건너뜀/번호 붙이기 |
| SOURCE_CHANGED | 미리보기 이후 원본이 바뀌었습니다 | 원본 유지, 재계획 필요 |
| FILE_BUSY | 다른 프로그램이 파일을 사용 중입니다 | 파일 실패, 계속 |
| PERMISSION_DENIED | 파일을 읽거나 쓸 권한이 없습니다 | 해당 항목/프로젝트 실패 |
| INSUFFICIENT_SPACE | 타겟의 저장 공간이 부족합니다 | 실행 중단 |
| VOLUME_DISCONNECTED | 연결된 저장장치에 접근할 수 없습니다 | 실행 중단 |
| VERIFY_FAILED | 복사한 파일 검증에 실패했습니다 | 원본 유지, 실패 |
| SOURCE_DELETE_FAILED | 복사는 완료됐지만 원본이 남아 있습니다 | sourceRetained |
| UNSUPPORTED_METADATA | 이 파일의 추가 메타데이터를 안전하게 옮길 수 없습니다 | 보류 |
| JOURNAL_WRITE_FAILED | 실행 기록을 안전하게 저장할 수 없습니다 | 신규 변경 중단 |
| SAVE_FAILED | 설정을 저장하지 못했습니다 | 미저장 표시 |
| REVISION_CONFLICT | 설정이 변경되어 다시 불러와야 합니다 | 저장 거부 |
| BUSY | 다른 이동 작업이 실행 중입니다 | 새 실행 거부 |

## 6. 디스크 구조

```text
<portable_root>/MoveMgrData/
  settings.json
  settings.backup.json
  runs/<runId>/summary.json
  runs/<runId>/journal.jsonl
  recovery/                   # 해결되지 않은 항목 요약
  plans/<planId>/             # 대규모 계획, 페이지 인덱스
```

계획 캐시는 실행 완료/만료 후 지울 수 있지만 journal은 별개다. 완료 이력은 최근 100회 또는 30일 중 먼저 도달한 범위까지 자동 정리하고, 미해결 sourceRetained/needsReview 및 미완료 실행은 자동 삭제하지 않는다. 사용자 데이터 경로는 Git이나 copy/ 전달본에 포함하지 않는다.

JSONL 마지막 줄이 비정상 종료로 잘렸으면 마지막 완전한 record까지 읽고 해당 항목을 복구 판정한다. journal record는 sequence, runId, itemId, state, timestamp, 원본/타겟 식별 정보, 검증 정보로 구성한다. 로그에 파일 본문을 쓰지 않는다.
