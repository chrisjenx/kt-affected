# Replay: element-hq/element-x-android

kt-affected 0.4.0 (5b516d0f7a), generated 2026-11-06T03:40:14+00:00. Merged pull requests 2026-08-07 .. 2026-11-05; CI runs from 2026-07-24.

## Summary

- Pull requests replayed: **501** (merge kinds: merge 309, rebase 21, squash 71, squash-or-rebase-1 100)
- Pull requests where a kt-affected call errored: 0
- Test classes per merge commit (CI tasks testDebugUnitTest, testGplayDebugUnitTest, test): 677 on average
- CI runs listed: 4245; failed or retried runs inspected: 1148; failed jobs: 648; logs read: 564 (expired: 84); report artifacts read: 170 (expired: 0)
- Failure rows on replayed PRs: 214 (213 with a test class); by attribution: a 19, b 25, b-build 8, c 161, task-only 1; by best source: html-report 29, log 185
- Diff-caused failures (a + b): **52** on 30 PRs

## Recall and cost

| Strategy | Diff-caused failures run (strict) | PRs with every such failure run | Mean classes selected | Mean share | Median share | Pooled share | Mean time share |
|---|---:|---:|---:|---:|---:|---:|---:|
| kt-affected | 47 / 52 (90.4%) | 28 / 30 | 135.0 | 19.9% | 14.8% | 20.0% | 21.8% |
| kt-affected+assist | 47 / 52 (90.4%) | 28 / 30 | 135.0 | 19.9% | 14.8% | 20.0% | 21.8% |
| module-level | 51 / 52 (98.1%) | 29 / 30 | 428.5 | 63.3% | 99.0% | 63.3% | 62.2% |
| run-everything | 52 / 52 (100.0%) | 30 / 30 | 676.7 | 100.0% | 100.0% | 100.0% | 100.0% |

Run-everything is what CI ran, so its recall is complete by construction; its row reports what the inventory holds. Shares count test classes of the CI tasks above; time shares price 783 classes from CI test reports (0 ms for unpriced classes).

## Failure rows

Attribution: **a** the failing test's file changed in the PR; **b** the test reaches a changed declaration (kt-affected path, or a hand check); **b-build** the failure follows from a build or dependency change in the PR (hand check); **c** unrelated (flake, environment, or caused by another change), excluded from recall; **review** not yet hand-checked, excluded; **task-only** a failing test task no source named a class for.

| PR | Module | Task | Class | Attribution | By | Sources | kt-affected | kt-affected+assist | module-level | run-everything | Note |
|---|---|---|---|---|---|---|---|---|---|---|---|
| [#6477](https://github.com/element-hq/element-x-android/pull/6477) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | a | auto | log | class | class | task | task |  |
| [#6477](https://github.com/element-hq/element-x-android/pull/6477) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:bxdxnn/element-x-android:misc/name-copy-fix, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7389 |
| [#6477](https://github.com/element-hq/element-x-android/pull/6477) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): branch:bxdxnn/element-x-android:misc/name-copy-fix, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages |
| [#6477](https://github.com/element-hq/element-x-android/pull/6477) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages |
| [#6477](https://github.com/element-hq/element-x-android/pull/6477) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages |
| [#7008](https://github.com/element-hq/element-x-android/pull/7008) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.pinned.list.PinnedMessagesListPresenterTest` | b | auto | html-report, log | class | class | task | task |  |
| [#7008](https://github.com/element-hq/element-x-android/pull/7008) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.threads.ThreadsListPresenterTest` | b | auto | html-report, log | class | class | task | task |  |
| [#7008](https://github.com/element-hq/element-x-android/pull/7008) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | b | auto | html-report, log | class | class | task | task |  |
| [#7008](https://github.com/element-hq/element-x-android/pull/7008) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.factories.TimelineItemsFactoryTest` | b-build | hand | html-report, log | **no** | **no** | task | task | The failing push was the bare androidx core 1.18 -> 1.19 catalog bump: toSpannable() NPEs in host tests under 1.19. The PR's next commit added safeToSpannable() to fix it. No other PR or base push failed this class. |
| [#7008](https://github.com/element-hq/element-x-android/pull/7008) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.voicemessages.composer.DefaultVoiceMessageComposerPresenterTest` | b | auto | html-report, log | class | class | task | task |  |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:features:lockscreen:impl` | testDebugUnitTest | `io.element.android.features.lockscreen.impl.unlock.keypad.PinKeypadTest` | a | auto | html-report, log | class | class | task | task |  |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | html-report, log | class | class | task | task | also failed on 3 other PR(s): branch:element-hq/element-x-android:renovate/androidx, #7644, #7663 |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7394, #7456, #7492 |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7492, #7538 |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7394, #7456, #7492 |
| [#7337](https://github.com/element-hq/element-x-android/pull/7337) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7394, #7456, #7492 |
| [#7389](https://github.com/element-hq/element-x-android/pull/7389) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:bxdxnn/element-x-android:misc/name-copy-fix, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #6477, #7394 |
| [#7394](https://github.com/element-hq/element-x-android/pull/7394) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistLicenseTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Use the correct license header on the new account provider history files"); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7394](https://github.com/element-hq/element-x-android/pull/7394) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7389, #7456 |
| [#7394](https://github.com/element-hq/element-x-android/pull/7394) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7456, #7492 |
| [#7394](https://github.com/element-hq/element-x-android/pull/7394) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7456, #7492 |
| [#7394](https://github.com/element-hq/element-x-android/pull/7394) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7456, #7492 |
| [#7410](https://github.com/element-hq/element-x-android/pull/7410) | `:features:home:impl` | testDebugUnitTest | `io.element.android.features.home.impl.search.GlobalSearchPresenterTest` | a | auto | html-report, log | task | task | task | task |  |
| [#7410](https://github.com/element-hq/element-x-android/pull/7410) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | class | class | task | task | also failed on 5 other PR(s): #7488, #7516, #7517, #7521, #7585; failed on 2 base-branch push run(s) |
| [#7410](https://github.com/element-hq/element-x-android/pull/7410) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPresenterTest` | b | auto | log | class | class | task | task |  |
| [#7450](https://github.com/element-hq/element-x-android/pull/7450) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | b | hand | log | task | task | task | task | Screenshot/accessibility test of a preview the PR changes; it failed on the PR's pushes and passed after the PR updated the view or its reference images. The other PR listed changed the same view. |
| [#7456](https://github.com/element-hq/element-x-android/pull/7456) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7389, #7394 |
| [#7456](https://github.com/element-hq/element-x-android/pull/7456) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7394, #7492 |
| [#7456](https://github.com/element-hq/element-x-android/pull/7456) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7394, #7492 |
| [#7456](https://github.com/element-hq/element-x-android/pull/7456) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:manfrommedan/element-x-android:multiselect-messages, #7337, #7394, #7492 |
| [#7458](https://github.com/element-hq/element-x-android/pull/7458) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | b | hand | log | task | task | task | task | Screenshot/accessibility test of a preview the PR changes; it failed on the PR's pushes and passed after the PR updated the view or its reference images. The other PR listed changed the same view. |
| [#7485](https://github.com/element-hq/element-x-android/pull/7485) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | log | **no** | **no** | task | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7008, #7519, #7579 |
| [#7488](https://github.com/element-hq/element-x-android/pull/7488) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | class | class | task | task | also failed on 4 other PR(s): #7410, #7516, #7517, #7521; failed on 2 base-branch push run(s) |
| [#7492](https://github.com/element-hq/element-x-android/pull/7492) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7394, #7456 |
| [#7492](https://github.com/element-hq/element-x-android/pull/7492) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7394, #7456 |
| [#7492](https://github.com/element-hq/element-x-android/pull/7492) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7394, #7456 |
| [#7492](https://github.com/element-hq/element-x-android/pull/7492) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 7 other PR(s): branch:bxdxnn/element-x-android:fix/thread-list-stutter, branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7394, #7456 |
| [#7516](https://github.com/element-hq/element-x-android/pull/7516) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | **no** | **no** | task | task | also failed on 5 other PR(s): #7410, #7488, #7517, #7521, #7585; failed on 2 base-branch push run(s) |
| [#7517](https://github.com/element-hq/element-x-android/pull/7517) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | class | class | task | task | also failed on 5 other PR(s): #7410, #7488, #7516, #7521, #7585; failed on 2 base-branch push run(s) |
| [#7519](https://github.com/element-hq/element-x-android/pull/7519) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | html-report, log | **no** | **no** | **no** | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7008, #7485, #7579 |
| [#7521](https://github.com/element-hq/element-x-android/pull/7521) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | class | class | **no** | task | also failed on 5 other PR(s): #7410, #7488, #7516, #7517, #7585; failed on 2 base-branch push run(s) |
| [#7538](https://github.com/element-hq/element-x-android/pull/7538) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7492, #7628 |
| [#7538](https://github.com/element-hq/element-x-android/pull/7538) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7492 |
| [#7538](https://github.com/element-hq/element-x-android/pull/7538) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7337, #7492 |
| [#7538](https://github.com/element-hq/element-x-android/pull/7538) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, branch:hayaksi1/element-x-android:fix/developer-options-switch, #7492 |
| [#7579](https://github.com/element-hq/element-x-android/pull/7579) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | html-report, log | **no** | **no** | task | task | also failed on 3 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7485, #7519; failed on 2 base-branch push run(s) |
| [#7585](https://github.com/element-hq/element-x-android/pull/7585) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | c | auto-flake | log | class | class | **no** | task | also failed on 4 other PR(s): #7410, #7516, #7517, #7521; failed on 2 base-branch push run(s) |
| [#7591](https://github.com/element-hq/element-x-android/pull/7591) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | a | auto | log | class | class | task | task |  |
| [#7626](https://github.com/element-hq/element-x-android/pull/7626) | `:enterprise:features:remoteconfig:impl` | testDebugUnitTest | `DefaultRemoteEnterpriseConfigRetrieverTest` | b | hand | log | **no** | **no** | task | task | The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests. |
| [#7626](https://github.com/element-hq/element-x-android/pull/7626) | `:enterprise:features:remoteconfig:impl` | testDebugUnitTest | `DefaultRemoteEnterpriseConfigStoreTest` | b | hand | log | **no** | **no** | task | task | The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests. |
| [#7626](https://github.com/element-hq/element-x-android/pull/7626) | `:enterprise:features:remoteconfig:impl` | testDebugUnitTest | `io.element.android.enterprise.features.remoteconfig.impl.DefaultRemoteEnterpriseConfigRetrieverTest` | b | hand | html-report | **no** | **no** | task | task | The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests. |
| [#7626](https://github.com/element-hq/element-x-android/pull/7626) | `:enterprise:features:remoteconfig:impl` | testDebugUnitTest | `io.element.android.enterprise.features.remoteconfig.impl.DefaultRemoteEnterpriseConfigStoreTest` | b | hand | html-report | **no** | **no** | task | task | The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests. |
| [#7628](https://github.com/element-hq/element-x-android/pull/7628) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/4388-media-upload-progress, #7337, #7538, #7657 |
| [#7644](https://github.com/element-hq/element-x-android/pull/7644) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | html-report, log | **no** | **no** | **no** | task | also failed on 3 other PR(s): branch:element-hq/element-x-android:renovate/androidx, #7337, #7663; failed on 2 base-branch push run(s) |
| [#7657](https://github.com/element-hq/element-x-android/pull/7657) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, #7337, #7538, #7628 |
| [#7663](https://github.com/element-hq/element-x-android/pull/7663) | `:features:messages:impl` | testDebugUnitTest | `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` | c | auto-flake | html-report, log | **no** | **no** | task | task | also failed on 3 other PR(s): branch:element-hq/element-x-android:renovate/androidx, #7337, #7644; failed on 2 base-branch push run(s) |
| [#7692](https://github.com/element-hq/element-x-android/pull/7692) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | b | hand | log | class | class | **no** | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Use PreviewParameterProvider", on the PR's new previews); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7698](https://github.com/element-hq/element-x-android/pull/7698) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistArchitectureTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Lint", on the PR's new VoiceMessageComposerPlayer); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7698](https://github.com/element-hq/element-x-android/pull/7698) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistTestTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Lint", renaming the PR's new tests); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7699](https://github.com/element-hq/element-x-android/pull/7699) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718; failed on 5 base-branch push run(s) |
| [#7700](https://github.com/element-hq/element-x-android/pull/7700) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7717, #7718; failed on 11 base-branch push run(s) |
| [#7700](https://github.com/element-hq/element-x-android/pull/7700) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7717, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7700](https://github.com/element-hq/element-x-android/pull/7700) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7700](https://github.com/element-hq/element-x-android/pull/7700) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7717, #7718, #7721, #7722, #7727; failed on 11 base-branch push run(s) |
| [#7703](https://github.com/element-hq/element-x-android/pull/7703) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistArchitectureTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Sealed interface must be immutable when uses in Composable function parameters"); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7717](https://github.com/element-hq/element-x-android/pull/7717) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7718; failed on 11 base-branch push run(s) |
| [#7717](https://github.com/element-hq/element-x-android/pull/7717) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7717](https://github.com/element-hq/element-x-android/pull/7717) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7718, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7717](https://github.com/element-hq/element-x-android/pull/7717) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7718, #7721, #7722, #7727; failed on 11 base-branch push run(s) |
| [#7718](https://github.com/element-hq/element-x-android/pull/7718) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 16 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717, #7721; failed on 11 base-branch push run(s) |
| [#7718](https://github.com/element-hq/element-x-android/pull/7718) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 11 other PR(s): #7756, #7757, #7759, #7781, #7798, #7803; failed on 12 base-branch push run(s) |
| [#7718](https://github.com/element-hq/element-x-android/pull/7718) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7718](https://github.com/element-hq/element-x-android/pull/7718) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7721, #7722, #7727; failed on 11 base-branch push run(s) |
| [#7721](https://github.com/element-hq/element-x-android/pull/7721) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 16 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717, #7718; failed on 11 base-branch push run(s) |
| [#7721](https://github.com/element-hq/element-x-android/pull/7721) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7722, #7727; failed on 11 base-branch push run(s) |
| [#7721](https://github.com/element-hq/element-x-android/pull/7721) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7722; failed on 11 base-branch push run(s) |
| [#7721](https://github.com/element-hq/element-x-android/pull/7721) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7722, #7727; failed on 11 base-branch push run(s) |
| [#7722](https://github.com/element-hq/element-x-android/pull/7722) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717; failed on 10 base-branch push run(s) |
| [#7722](https://github.com/element-hq/element-x-android/pull/7722) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721; failed on 10 base-branch push run(s) |
| [#7722](https://github.com/element-hq/element-x-android/pull/7722) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 10 base-branch push run(s) |
| [#7722](https://github.com/element-hq/element-x-android/pull/7722) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7727; failed on 10 base-branch push run(s) |
| [#7728](https://github.com/element-hq/element-x-android/pull/7728) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717; failed on 11 base-branch push run(s) |
| [#7728](https://github.com/element-hq/element-x-android/pull/7728) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721; failed on 11 base-branch push run(s) |
| [#7728](https://github.com/element-hq/element-x-android/pull/7728) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 11 base-branch push run(s) |
| [#7728](https://github.com/element-hq/element-x-android/pull/7728) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7729](https://github.com/element-hq/element-x-android/pull/7729) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | class | class | task | task | also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717; failed on 11 base-branch push run(s) |
| [#7729](https://github.com/element-hq/element-x-android/pull/7729) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721; failed on 11 base-branch push run(s) |
| [#7729](https://github.com/element-hq/element-x-android/pull/7729) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 11 base-branch push run(s) |
| [#7729](https://github.com/element-hq/element-x-android/pull/7729) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7730](https://github.com/element-hq/element-x-android/pull/7730) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 16 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717, #7718; failed on 11 base-branch push run(s) |
| [#7730](https://github.com/element-hq/element-x-android/pull/7730) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7730](https://github.com/element-hq/element-x-android/pull/7730) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 11 base-branch push run(s) |
| [#7730](https://github.com/element-hq/element-x-android/pull/7730) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s) |
| [#7731](https://github.com/element-hq/element-x-android/pull/7731) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 16 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717, #7718; failed on 11 base-branch push run(s) |
| [#7735](https://github.com/element-hq/element-x-android/pull/7735) | `:libraries:compound` | testDebugUnitTest | `io.element.android.compound.screenshot.LegacyColorsTest` | a | auto | log | task | task | task | task |  |
| [#7735](https://github.com/element-hq/element-x-android/pull/7735) | `:libraries:compound` | testDebugUnitTest | `io.element.android.compound.screenshot.MaterialColorSchemeTest` | a | auto | log | task | task | task | task |  |
| [#7735](https://github.com/element-hq/element-x-android/pull/7735) | `:libraries:compound` | testDebugUnitTest | `io.element.android.compound.screenshot.SemanticColorsTest` | a | auto | log | task | task | task | task |  |
| [#7736](https://github.com/element-hq/element-x-android/pull/7736) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 17 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717, #7718; failed on 11 base-branch push run(s) |
| [#7750](https://github.com/element-hq/element-x-android/pull/7750) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | b | hand | log | task | task | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Add missing ElementPreview wrapper"); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7756](https://github.com/element-hq/element-x-android/pull/7756) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | class | class | task | task | also failed on 21 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7756](https://github.com/element-hq/element-x-android/pull/7756) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 17 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 17 base-branch push run(s) |
| [#7756](https://github.com/element-hq/element-x-android/pull/7756) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7756](https://github.com/element-hq/element-x-android/pull/7756) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7757](https://github.com/element-hq/element-x-android/pull/7757) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 21 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7757](https://github.com/element-hq/element-x-android/pull/7757) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 17 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 17 base-branch push run(s) |
| [#7757](https://github.com/element-hq/element-x-android/pull/7757) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7757](https://github.com/element-hq/element-x-android/pull/7757) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s) |
| [#7759](https://github.com/element-hq/element-x-android/pull/7759) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7718, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7759](https://github.com/element-hq/element-x-android/pull/7759) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7718, #7781, #7798, #7803, #7816, #7817; failed on 6 base-branch push run(s) |
| [#7759](https://github.com/element-hq/element-x-android/pull/7759) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7337, #7718, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7759](https://github.com/element-hq/element-x-android/pull/7759) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7718, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7761](https://github.com/element-hq/element-x-android/pull/7761) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files; the preview-naming rule failed on the PR's new previews and passed on its later pushes. The other PR that failed it shares the branch's commits. A source-scanning test observes every source file. |
| [#7761](https://github.com/element-hq/element-x-android/pull/7761) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | class | class | task | task | also failed on 3 other PR(s): #7904, #7911, #7928 |
| [#7761](https://github.com/element-hq/element-x-android/pull/7761) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | class | class | task | task | also failed on 3 other PR(s): branch:element-hq/element-x-android:sync-localazy, #7904, #7968 |
| [#7781](https://github.com/element-hq/element-x-android/pull/7781) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | class | class | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7787; failed on 12 base-branch push run(s) |
| [#7781](https://github.com/element-hq/element-x-android/pull/7781) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7798, #7803; failed on 12 base-branch push run(s) |
| [#7781](https://github.com/element-hq/element-x-android/pull/7781) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7798; failed on 12 base-branch push run(s) |
| [#7781](https://github.com/element-hq/element-x-android/pull/7781) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7798; failed on 12 base-branch push run(s) |
| [#7787](https://github.com/element-hq/element-x-android/pull/7787) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7337, #7718, #7736, #7756, #7757, #7781; failed on 12 base-branch push run(s) |
| [#7798](https://github.com/element-hq/element-x-android/pull/7798) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | class | class | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7798](https://github.com/element-hq/element-x-android/pull/7798) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7803; failed on 12 base-branch push run(s) |
| [#7798](https://github.com/element-hq/element-x-android/pull/7798) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7798](https://github.com/element-hq/element-x-android/pull/7798) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | task | task | also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7800](https://github.com/element-hq/element-x-android/pull/7800) | `:features:home:impl` | testDebugUnitTest | `io.element.android.features.home.impl.roomlist.RoomListPresenterTest` | a | auto | html-report, log | class | class | task | task | also failed on 1 other PR(s): #7803 |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:features:home:impl` | testDebugUnitTest | `io.element.android.features.home.impl.roomlist.RoomListPresenterTest` | a | auto | html-report, log | task | task | task | task | also failed on 1 other PR(s): #7800 |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7823, #7826 |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7798; failed on 12 base-branch push run(s) |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7803](https://github.com/element-hq/element-x-android/pull/7803) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:app` | testGplayDebugUnitTest | `io.element.android.x.intent.DefaultIntentProviderTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:appnav` | testDebugUnitTest | `io.element.android.appnav.JoinedRoomLoadedFlowNodeTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:appnav` | testDebugUnitTest | `io.element.android.appnav.RootFlowNodeTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:appnav` | testDebugUnitTest | `io.element.android.appnav.intent.IntentResolverTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:libraries:androidutils` | testDebugUnitTest | `io.element.android.libraries.androidutils.media.VideoCompressorHelperTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:libraries:androidutils` | testDebugUnitTest | `io.element.android.libraries.androidutils.text.LinkifierHelperTest` | b-build | hand | html-report, log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7815](https://github.com/element-hq/element-x-android/pull/7815) | `:libraries:compound` | testDebugUnitTest | `io.element.android.compound.theme.ThemeTest` | b-build | hand | log | task | task | task | task | The PR bumps Robolectric to 4.17 (version catalog, and test JVM args in build.gradle.kts); Robolectric-backed classes across modules failed with IllegalAccessException until the PR adjusted the JVM flags. No other PR or base push failed these classes. |
| [#7816](https://github.com/element-hq/element-x-android/pull/7816) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7816](https://github.com/element-hq/element-x-android/pull/7816) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7798; failed on 12 base-branch push run(s) |
| [#7816](https://github.com/element-hq/element-x-android/pull/7816) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7816](https://github.com/element-hq/element-x-android/pull/7816) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7817](https://github.com/element-hq/element-x-android/pull/7817) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7817](https://github.com/element-hq/element-x-android/pull/7817) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7798; failed on 12 base-branch push run(s) |
| [#7817](https://github.com/element-hq/element-x-android/pull/7817) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7817](https://github.com/element-hq/element-x-android/pull/7817) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | **no** | **no** | **no** | task | also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s) |
| [#7823](https://github.com/element-hq/element-x-android/pull/7823) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7803, #7826 |
| [#7823](https://github.com/element-hq/element-x-android/pull/7823) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7337, #7718, #7759, #7781, #7787, #7798; failed on 6 base-branch push run(s) |
| [#7823](https://github.com/element-hq/element-x-android/pull/7823) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 11 other PR(s): #7718, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7823](https://github.com/element-hq/element-x-android/pull/7823) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7337, #7718, #7759, #7781, #7798, #7803; failed on 6 base-branch push run(s) |
| [#7823](https://github.com/element-hq/element-x-android/pull/7823) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 11 other PR(s): #7337, #7718, #7759, #7781, #7798, #7803; failed on 6 base-branch push run(s) |
| [#7826](https://github.com/element-hq/element-x-android/pull/7826) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7803, #7823 |
| [#7839](https://github.com/element-hq/element-x-android/pull/7839) | `:libraries:roomselect:impl` | testDebugUnitTest | `io.element.android.libraries.roomselect.impl.RoomSelectPresenterTest` | a | auto | html-report, log | task | task | task | task |  |
| [#7840](https://github.com/element-hq/element-x-android/pull/7840) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7840](https://github.com/element-hq/element-x-android/pull/7840) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7759, #7781, #7798, #7803, #7816, #7817; failed on 6 base-branch push run(s) |
| [#7840](https://github.com/element-hq/element-x-android/pull/7840) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7840](https://github.com/element-hq/element-x-android/pull/7840) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7841](https://github.com/element-hq/element-x-android/pull/7841) | `:features:home:impl` | testDebugUnitTest | `io.element.android.features.home.impl.components.RoomSummaryRowUnreadIndicatorCountTest` | a | auto | html-report, log | class | class | task | task |  |
| [#7853](https://github.com/element-hq/element-x-android/pull/7853) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7718, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7853](https://github.com/element-hq/element-x-android/pull/7853) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 14 other PR(s): #7337, #7718, #7759, #7781, #7798, #7803; failed on 6 base-branch push run(s) |
| [#7864](https://github.com/element-hq/element-x-android/pull/7864) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistClassNameTest` | a | auto | log | class | class | task | task | also failed on 1 other PR(s): branch:element-hq/element-x-android:feature/ptt-settings-navigation |
| [#7864](https://github.com/element-hq/element-x-android/pull/7864) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistImmutableTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Use the Element Creations license header on the new selection files"); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7864](https://github.com/element-hq/element-x-android/pull/7864) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistLicenseTest` | b | hand | log | class | class | task | task | Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Use the Element Creations license header on the new selection files"); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it. |
| [#7864](https://github.com/element-hq/element-x-android/pull/7864) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | a | auto | log | class | class | task | task | also failed on 1 other PR(s): #7865 |
| [#7865](https://github.com/element-hq/element-x-android/pull/7865) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | b | hand | log | task | task | task | task | Konsist preview-naming rule failed on four pushes of the PR (and of #7864, which it was stacked on) and passed after; no push to the base branch failed it. |
| [#7865](https://github.com/element-hq/element-x-android/pull/7865) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7865](https://github.com/element-hq/element-x-android/pull/7865) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7759, #7781, #7798, #7803, #7816, #7817; failed on 6 base-branch push run(s) |
| [#7865](https://github.com/element-hq/element-x-android/pull/7865) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7865](https://github.com/element-hq/element-x-android/pull/7865) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 12 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7869](https://github.com/element-hq/element-x-android/pull/7869) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7869](https://github.com/element-hq/element-x-android/pull/7869) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7759, #7781, #7798, #7803, #7816, #7817; failed on 6 base-branch push run(s) |
| [#7869](https://github.com/element-hq/element-x-android/pull/7869) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7869](https://github.com/element-hq/element-x-android/pull/7869) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 13 other PR(s): #7337, #7759, #7781, #7798, #7803, #7816; failed on 6 base-branch push run(s) |
| [#7883](https://github.com/element-hq/element-x-android/pull/7883) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 8 other PR(s): #7759, #7840, #7865, #7869, #7896, #7904 |
| [#7883](https://github.com/element-hq/element-x-android/pull/7883) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 9 other PR(s): #7759, #7840, #7853, #7865, #7869, #7896 |
| [#7883](https://github.com/element-hq/element-x-android/pull/7883) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 9 other PR(s): #7759, #7840, #7853, #7865, #7869, #7896 |
| [#7883](https://github.com/element-hq/element-x-android/pull/7883) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): #7759, #7840, #7865, #7869, #7896, #7904 |
| [#7890](https://github.com/element-hq/element-x-android/pull/7890) | `:libraries:matrix:impl` | testDebugUnitTest | `io.element.android.libraries.matrix.impl.RustMatrixClientFactoryTest` | a | auto | html-report, log | class | class | task | task |  |
| [#7890](https://github.com/element-hq/element-x-android/pull/7890) | `:libraries:matrix:impl` | testDebugUnitTest | `io.element.android.libraries.matrix.impl.auth.RustMatrixAuthenticationServiceTest` | b | auto | html-report, log | class | class | task | task |  |
| [#7896](https://github.com/element-hq/element-x-android/pull/7896) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 8 other PR(s): #7759, #7840, #7865, #7869, #7883, #7904 |
| [#7896](https://github.com/element-hq/element-x-android/pull/7896) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 9 other PR(s): #7759, #7840, #7853, #7865, #7869, #7883 |
| [#7896](https://github.com/element-hq/element-x-android/pull/7896) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 9 other PR(s): #7759, #7840, #7853, #7865, #7869, #7883 |
| [#7896](https://github.com/element-hq/element-x-android/pull/7896) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): #7759, #7840, #7865, #7869, #7883, #7904 |
| [#7901](https://github.com/element-hq/element-x-android/pull/7901) | `:features:preferences:impl` | testDebugUnitTest | `io.element.android.features.preferences.impl.notifications.NotificationSettingsPresenterTest` | a | auto | html-report, log | class | class | task | task |  |
| [#7904](https://github.com/element-hq/element-x-android/pull/7904) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewA11yTest` | b | hand | log | task | task | task | task | Screenshot/accessibility test of a preview the PR changes; it failed on the PR's pushes and passed after the PR updated the view or its reference images. The other PR listed changed the same view. |
| [#7904](https://github.com/element-hq/element-x-android/pull/7904) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): #7840, #7869, #7883, #7896, #7911, #7928 |
| [#7904](https://github.com/element-hq/element-x-android/pull/7904) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): #7840, #7869, #7883, #7896, #7922, #7928 |
| [#7904](https://github.com/element-hq/element-x-android/pull/7904) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:element-hq/element-x-android:sync-localazy, #7883, #7896, #7922, #7928 |
| [#7904](https://github.com/element-hq/element-x-android/pull/7904) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:element-hq/element-x-android:sync-localazy, #7840, #7869, #7883, #7896 |
| [#7911](https://github.com/element-hq/element-x-android/pull/7911) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 6 other PR(s): #7761, #7840, #7883, #7896, #7904, #7928 |
| [#7922](https://github.com/element-hq/element-x-android/pull/7922) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): #7883, #7896, #7904, #7928 |
| [#7922](https://github.com/element-hq/element-x-android/pull/7922) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 5 other PR(s): branch:element-hq/element-x-android:sync-localazy, #7883, #7896, #7904, #7928 |
| [#7966](https://github.com/element-hq/element-x-android/pull/7966) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistPreviewTest` | a | auto | log | class | class | task | task | also failed on 1 other PR(s): #7761 |
| [#7966](https://github.com/element-hq/element-x-android/pull/7966) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7967, #7968, #8024 |
| [#7966](https://github.com/element-hq/element-x-android/pull/7966) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7967, #8024 |
| [#7967](https://github.com/element-hq/element-x-android/pull/7967) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #8024, #8078 |
| [#7967](https://github.com/element-hq/element-x-android/pull/7967) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): #7966, #7968, #8024, #8078 |
| [#7967](https://github.com/element-hq/element-x-android/pull/7967) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7966, #8024, #8078 |
| [#7967](https://github.com/element-hq/element-x-android/pull/7967) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7968, #8024, #8078 |
| [#7968](https://github.com/element-hq/element-x-android/pull/7968) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7966, #7967, #8024 |
| [#7968](https://github.com/element-hq/element-x-android/pull/7968) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): branch:element-hq/element-x-android:sync-localazy, #7761, #7967, #8024 |
| [#7981](https://github.com/element-hq/element-x-android/pull/7981) | `:tests:konsist` | testDebugUnitTest | `-` | task-only | auto | log | **no** | **no** | **no** | task |  |
| [#8022](https://github.com/element-hq/element-x-android/pull/8022) | `:libraries:matrix:impl` | testDebugUnitTest | `io.element.android.libraries.matrix.impl.room.RoomInfoExtTest` | a | auto | html-report, log | class | class | task | task |  |
| [#8024](https://github.com/element-hq/element-x-android/pull/8024) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7967, #8078 |
| [#8024](https://github.com/element-hq/element-x-android/pull/8024) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 4 other PR(s): #7966, #7967, #7968, #8078 |
| [#8024](https://github.com/element-hq/element-x-android/pull/8024) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7966, #7967, #8078 |
| [#8024](https://github.com/element-hq/element-x-android/pull/8024) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7967, #7968, #8078 |
| [#8052](https://github.com/element-hq/element-x-android/pull/8052) | `:tests:konsist` | testDebugUnitTest | `io.element.android.tests.konsist.KonsistComposableTest` | a | auto | log | class | class | task | task |  |
| [#8078](https://github.com/element-hq/element-x-android/pull/8078) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard1Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7967, #8024 |
| [#8078](https://github.com/element-hq/element-x-android/pull/8078) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard2Test` | c | auto-flake | log | task | task | task | task | also failed on 2 other PR(s): #7967, #8024 |
| [#8078](https://github.com/element-hq/element-x-android/pull/8078) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7967, #8024, #8105 |
| [#8078](https://github.com/element-hq/element-x-android/pull/8078) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | c | auto-flake | log | task | task | task | task | also failed on 3 other PR(s): #7967, #8024, #8105 |
| [#8079](https://github.com/element-hq/element-x-android/pull/8079) | `:features:home:impl` | testDebugUnitTest | `io.element.android.features.home.impl.search.GlobalSearchViewTest` | a | auto | html-report, log | class | class | task | task |  |
| [#8105](https://github.com/element-hq/element-x-android/pull/8105) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard3Test` | b | hand | log | task | task | task | task | Screenshot/accessibility test of a preview the PR changes; it failed on the PR's pushes and passed after the PR updated the view or its reference images. The other PR listed changed the same view. |
| [#8105](https://github.com/element-hq/element-x-android/pull/8105) | `:tests:uitests` | testDebugUnitTest | `ui.PreviewShard4Test` | b | hand | log | task | task | task | task | Screenshot/accessibility test of a preview the PR changes; it failed on the PR's pushes and passed after the PR updated the view or its reference images. The other PR listed changed the same view. |

## Rows a selector misses

Every class row some kt-affected strategy or the module-level baseline does not run, whatever its attribution, so nothing is excluded unseen.

### #7008 `io.element.android.features.messages.impl.timeline.factories.TimelineItemsFactoryTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `9efa76e5fa29cad40e0c8d86047fc278b724b4b6`, base `69f1af6ef498c457a44bad67061250d06876bb1b` (rebase)
- Missed by: kt-affected, kt-affected+assist; attribution **b-build** (hand)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/factories/TimelineItemsFactoryTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32253568869 attempt 1 (log, b67d03ee7f), run 32253568869 attempt None (html-report, b67d03ee7f)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 2: `libraries/androidutils/src/main/kotlin/io/element/android/libraries/androidutils/text/LinkifyHelper.kt: LinkifyHelper.linkify`; `libraries/androidutils/src/main/kotlin/io/element/android/libraries/androidutils/text/TextUtils.kt: safeToSpannable`
- Hand check: The failing push was the bare androidx core 1.18 -> 1.19 catalog bump: toSpannable() NPEs in host tests under 1.19. The PR's next commit added safeToSpannable() to fix it. No other PR or base push failed this class.

### #7485 `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `4fa9576029a1631ea6aa4cc582a5494f59ddd85b`, base `2525019ca1974bb44308cb09015327305506d82b` (squash-or-rebase-1)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/TimelinePresenterTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32317134891 attempt 1 (log, 879523936c)
- Flake evidence: also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7008, #7519, #7579
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7519 `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `cc6827b827cbf624bb31a9d2ca87d5e41716c260`, base `f740595ad5e4c86786e41a2fc53e317a036a91ec` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/TimelinePresenterTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32748154561 attempt 1 (log, 2226eb5162), run 32748154561 attempt None (html-report, 2226eb5162)
- Flake evidence: also failed on 4 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7008, #7485, #7579
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7579 `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `103c60696839b811716b6243989fe1e842ae3ea2`, base `4800bbc63fd673f4d7132a097e065f24076c401e` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/TimelinePresenterTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32990976148 attempt 1 (log, f051966b74), run 32990976148 attempt None (html-report, f051966b74)
- Flake evidence: also failed on 3 other PR(s): branch:hayaksi1/element-x-android:fix/7062-invitations-under-notify-me, #7485, #7519; failed on 2 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7626 `DefaultRemoteEnterpriseConfigRetrieverTest` (:enterprise:features:remoteconfig:impl:testDebugUnitTest)

- Merge commit `3b2e416855d2e81f53c075ec2900b9603d78bdd5`, base `da44cc40756d2e366d2c71bde41d34152544b85d` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **b** (hand)
- Test file: not found at the merge commit; changed in the PR: no; in kt-affected's inventory: no
- Failed on: run 33613803990 attempt 1 (log, d016c2c23c)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 0
- Hand check: The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests.

### #7626 `DefaultRemoteEnterpriseConfigStoreTest` (:enterprise:features:remoteconfig:impl:testDebugUnitTest)

- Merge commit `3b2e416855d2e81f53c075ec2900b9603d78bdd5`, base `da44cc40756d2e366d2c71bde41d34152544b85d` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **b** (hand)
- Test file: not found at the merge commit; changed in the PR: no; in kt-affected's inventory: no
- Failed on: run 33613803990 attempt 1 (log, d016c2c23c)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 0
- Hand check: The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests.

### #7626 `io.element.android.enterprise.features.remoteconfig.impl.DefaultRemoteEnterpriseConfigRetrieverTest` (:enterprise:features:remoteconfig:impl:testDebugUnitTest)

- Merge commit `3b2e416855d2e81f53c075ec2900b9603d78bdd5`, base `da44cc40756d2e366d2c71bde41d34152544b85d` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **b** (hand)
- Test file: not found at the merge commit; changed in the PR: no; in kt-affected's inventory: no
- Failed on: run 33613803990 attempt None (html-report, d016c2c23c)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 0
- Hand check: The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests.

### #7626 `io.element.android.enterprise.features.remoteconfig.impl.DefaultRemoteEnterpriseConfigStoreTest` (:enterprise:features:remoteconfig:impl:testDebugUnitTest)

- Merge commit `3b2e416855d2e81f53c075ec2900b9603d78bdd5`, base `da44cc40756d2e366d2c71bde41d34152544b85d` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **b** (hand)
- Test file: not found at the merge commit; changed in the PR: no; in kt-affected's inventory: no
- Failed on: run 33613803990 attempt None (html-report, d016c2c23c)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 0
- Hand check: The PR moves the enterprise submodule pointer ("Fix fetched remote config being cached with the wrong key"); the failing classes live in that submodule, which is private, so the replay's checkout has no sources for them. Diff-caused, but no tool without submodule access can see these tests.

### #7644 `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `0e12d4e119a713de5428731555dbbf6a818cbfb0`, base `5c0c330bbd724bfc0e7a3a4c1929d2570f7c3b0c` (squash-or-rebase-1)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/TimelinePresenterTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 33854616449 attempt 1 (log, 768666a83d), run 33854616449 attempt None (html-report, 768666a83d)
- Flake evidence: also failed on 3 other PR(s): branch:element-hq/element-x-android:renovate/androidx, #7337, #7663; failed on 2 base-branch push run(s)
- Changed declarations the test file names: `present`
- Functional changed declarations: 13: `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationPresenter.kt: IncomingVerificationPresenter`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationPresenter.kt: IncomingVerificationPresenter.sessionVerificationService`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationPresenter.kt: IncomingVerificationPresenter.stateMachine`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationPresenter.kt: IncomingVerificationPresenter.present`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationState.kt: IncomingVerificationState.Step`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationState.kt: IncomingVerificationState.Step.Canceled`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationStatePreviewParam.kt: IncomingVerificationStatePreviewParam`; `features/verifysession/impl/src/main/kotlin/io/element/android/features/verifysession/impl/incoming/IncomingVerificationStatePreviewParam.kt: IncomingVerificationStatePreviewParam.values`

### #7663 `io.element.android.features.messages.impl.timeline.TimelinePresenterTest` (:features:messages:impl:testDebugUnitTest)

- Merge commit `875ff9825b807ec226c59b79bce504a0883cabc7`, base `3df1906d2a2bac789870221463636d729fb2212b` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `features/messages/impl/src/test/kotlin/io/element/android/features/messages/impl/timeline/TimelinePresenterTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 34146407912 attempt 1 (log, 6c01ad1364), run 34146407912 attempt None (html-report, 6c01ad1364)
- Flake evidence: also failed on 3 other PR(s): branch:element-hq/element-x-android:renovate/androidx, #7337, #7644; failed on 2 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 9: `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/MapDefaults.kt: MapDefaults`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/MapDefaults.kt: MapDefaults.options`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/ui/MapBottomSheetScaffold.kt: MapBottomSheetScaffold`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/userlocation/DefaultUserLocationStateFactory.kt: DefaultUserLocationStateFactory.create`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/userlocation/LocationProvider.kt: (whole file)`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/common/userlocation/UserLocationState.kt: UserLocationState`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/live/service/LiveLocationSharingService.kt: LiveLocationSharingService.startLocationUpdatesListener`; `features/location/impl/src/main/kotlin/io/element/android/features/location/impl/share/ShareLocationView.kt: ShareLocationView`

### #7717 `ui.PreviewShard1Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `fc63d95b655e80d17a559403d16b5ba518828951`, base `3fc5a561895417aee544bbf8748334f9184f1b70` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard1Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35110191559 attempt 1 (log, b854e5cc01)
- Flake evidence: also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7718; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7717 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `fc63d95b655e80d17a559403d16b5ba518828951`, base `3fc5a561895417aee544bbf8748334f9184f1b70` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35110191559 attempt 1 (log, b854e5cc01)
- Flake evidence: also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7721, #7722; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7717 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `fc63d95b655e80d17a559403d16b5ba518828951`, base `3fc5a561895417aee544bbf8748334f9184f1b70` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35110191559 attempt 1 (log, b854e5cc01)
- Flake evidence: also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7718, #7721, #7722; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7717 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `fc63d95b655e80d17a559403d16b5ba518828951`, base `3fc5a561895417aee544bbf8748334f9184f1b70` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35110191559 attempt 1 (log, b854e5cc01)
- Flake evidence: also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7718, #7721, #7722, #7727; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7729 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `f5ed93b4742fc64c533c2c84df43d508ec8095a2`, base `6642383d03de3da2e79d8bd07d6651619cde5c40` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223821337 attempt 1 (log, 47b8261fb2)
- Flake evidence: also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7729 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `f5ed93b4742fc64c533c2c84df43d508ec8095a2`, base `6642383d03de3da2e79d8bd07d6651619cde5c40` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223821337 attempt 1 (log, 47b8261fb2)
- Flake evidence: also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7729 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `f5ed93b4742fc64c533c2c84df43d508ec8095a2`, base `6642383d03de3da2e79d8bd07d6651619cde5c40` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223821337 attempt 1 (log, 47b8261fb2)
- Flake evidence: also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7728 `ui.PreviewShard1Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `82c841a6b04b94649cc3905dd04f9e54c6c1d5ec`, base `47620e905fff93405163ab66395ffe61771bb5c6` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard1Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223804494 attempt 1 (log, ca753ee600)
- Flake evidence: also failed on 17 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7699, #7700, #7717; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7728 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `82c841a6b04b94649cc3905dd04f9e54c6c1d5ec`, base `47620e905fff93405163ab66395ffe61771bb5c6` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223804494 attempt 1 (log, ca753ee600)
- Flake evidence: also failed on 13 other PR(s): branch:bxdxnn/element-x-android:feat/smooth-msgs, branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7721; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7728 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `82c841a6b04b94649cc3905dd04f9e54c6c1d5ec`, base `47620e905fff93405163ab66395ffe61771bb5c6` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223804494 attempt 1 (log, ca753ee600)
- Flake evidence: also failed on 13 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7728 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `82c841a6b04b94649cc3905dd04f9e54c6c1d5ec`, base `47620e905fff93405163ab66395ffe61771bb5c6` (merge)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35223804494 attempt 1 (log, ca753ee600)
- Flake evidence: also failed on 12 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 11 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7756 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `e5c078c01a7eda20114bba810336a50bafaa3891`, base `72d828f2ec3a48b6f2621d768599661fad85e0c4` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35849012476 attempt 1 (log, 2ab3c7ec53)
- Flake evidence: also failed on 17 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7700, #7717, #7718, #7721, #7722; failed on 17 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7756 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `e5c078c01a7eda20114bba810336a50bafaa3891`, base `72d828f2ec3a48b6f2621d768599661fad85e0c4` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35849012476 attempt 1 (log, 2ab3c7ec53)
- Flake evidence: also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7756 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `e5c078c01a7eda20114bba810336a50bafaa3891`, base `72d828f2ec3a48b6f2621d768599661fad85e0c4` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 35849012476 attempt 1 (log, 2ab3c7ec53)
- Flake evidence: also failed on 18 other PR(s): branch:element-hq/element-x-android:feature/bma/fixScreenshotFlakiness, #7337, #7700, #7717, #7718, #7721; failed on 17 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7781 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `0f29e8e8753514a4b0cd87be79103aa294c168f8`, base `439898a74c63207bd1b3f29c8b37eaecf6e6540e` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36613637971 attempt 1 (log, 8195a4d637)
- Flake evidence: also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7798, #7803; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 9: `features/roomdetails/api/src/main/kotlin/io/element/android/features/roomdetails/api/RoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.roomDetailsExtension`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.View`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsView.kt: RoomDetailsView`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtensionTest.kt: (whole file)`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsViewTest.kt: RoomDetailsViewTest.extension section is rendered`

### #7781 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `0f29e8e8753514a4b0cd87be79103aa294c168f8`, base `439898a74c63207bd1b3f29c8b37eaecf6e6540e` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36613637971 attempt 1 (log, 8195a4d637)
- Flake evidence: also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7798; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 9: `features/roomdetails/api/src/main/kotlin/io/element/android/features/roomdetails/api/RoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.roomDetailsExtension`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.View`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsView.kt: RoomDetailsView`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtensionTest.kt: (whole file)`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsViewTest.kt: RoomDetailsViewTest.extension section is rendered`

### #7781 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `0f29e8e8753514a4b0cd87be79103aa294c168f8`, base `439898a74c63207bd1b3f29c8b37eaecf6e6540e` (squash)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36613637971 attempt 1 (log, 8195a4d637)
- Flake evidence: also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7798; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 9: `features/roomdetails/api/src/main/kotlin/io/element/android/features/roomdetails/api/RoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtension.kt: (whole file)`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.roomDetailsExtension`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsNode.kt: RoomDetailsNode.View`; `features/roomdetails/impl/src/main/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsView.kt: RoomDetailsView`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/NoopRoomDetailsExtensionTest.kt: (whole file)`; `features/roomdetails/impl/src/test/kotlin/io/element/android/features/roomdetails/impl/RoomDetailsViewTest.kt: RoomDetailsViewTest.extension section is rendered`

### #7817 `ui.PreviewShard1Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `55b0273a4bb7c8b53f4f33f8a2ed2bf3b881b027`, base `a2b2c2360ad527ee97cab720ed5c0a31e82e969a` (squash)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard1Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36602036765 attempt 1 (log, 5b6c70c8f2)
- Flake evidence: also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 10: `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.dispatchers`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.isElementProEnforced`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.accountProviderAllowList`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.canConnectToAnyAccountProvider is always true`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.createDefaultEnterpriseService`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection.dispatchers`

### #7817 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `55b0273a4bb7c8b53f4f33f8a2ed2bf3b881b027`, base `a2b2c2360ad527ee97cab720ed5c0a31e82e969a` (squash)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36602036765 attempt 1 (log, 5b6c70c8f2)
- Flake evidence: also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7798; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 10: `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.dispatchers`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.isElementProEnforced`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.accountProviderAllowList`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.canConnectToAnyAccountProvider is always true`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.createDefaultEnterpriseService`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection.dispatchers`

### #7817 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `55b0273a4bb7c8b53f4f33f8a2ed2bf3b881b027`, base `a2b2c2360ad527ee97cab720ed5c0a31e82e969a` (squash)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36602036765 attempt 1 (log, 5b6c70c8f2)
- Flake evidence: also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 10: `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.dispatchers`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.isElementProEnforced`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.accountProviderAllowList`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.canConnectToAnyAccountProvider is always true`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.createDefaultEnterpriseService`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection.dispatchers`

### #7817 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `55b0273a4bb7c8b53f4f33f8a2ed2bf3b881b027`, base `a2b2c2360ad527ee97cab720ed5c0a31e82e969a` (squash)
- Missed by: kt-affected, kt-affected+assist, module-level; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36602036765 attempt 1 (log, 5b6c70c8f2)
- Flake evidence: also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 10: `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.dispatchers`; `features/enterprise/impl-foss/src/main/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseService.kt: DefaultEnterpriseService.isElementProEnforced`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.accountProviderAllowList`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.canConnectToAnyAccountProvider is always true`; `features/enterprise/impl-foss/src/test/kotlin/io/element/android/features/enterprise/impl/DefaultEnterpriseServiceTest.kt: DefaultEnterpriseServiceTest.createDefaultEnterpriseService`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection`; `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/classic/ElementClassicConnection.kt: DefaultElementClassicConnection.dispatchers`

### #7798 `ui.PreviewShard2Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `3ae227946bc8933f2c912e6b62eb1457096f74df`, base `3b5696d37dad7e7d3f2adef1060a4667b0c9c790` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard2Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36600471208 attempt 1 (log, 7dfa5666a5)
- Flake evidence: also failed on 14 other PR(s): #7718, #7756, #7757, #7759, #7781, #7803; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7798 `ui.PreviewShard3Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `3ae227946bc8933f2c912e6b62eb1457096f74df`, base `3b5696d37dad7e7d3f2adef1060a4667b0c9c790` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard3Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36600471208 attempt 1 (log, 7dfa5666a5)
- Flake evidence: also failed on 15 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7798 `ui.PreviewShard4Test` (:tests:uitests:testDebugUnitTest)

- Merge commit `3ae227946bc8933f2c912e6b62eb1457096f74df`, base `3b5696d37dad7e7d3f2adef1060a4667b0c9c790` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/uitests/src/test/kotlin/ui/PreviewShard4Test.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 36600471208 attempt 1 (log, 7dfa5666a5)
- Flake evidence: also failed on 14 other PR(s): #7337, #7718, #7756, #7757, #7759, #7781; failed on 12 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7521 `io.element.android.tests.konsist.KonsistClassNameTest` (:tests:konsist:testDebugUnitTest)

- Merge commit `872319c8ea7feabe79d3aa8be61103ce9c10c91c`, base `23437e44278751c4409587cd77569378e5882814` (merge)
- Missed by: module-level; attribution **c** (auto-flake)
- Test file: `tests/konsist/src/test/kotlin/io/element/android/tests/konsist/KonsistClassNameTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32763891576 attempt 1 (log, feb87ead8d)
- Flake evidence: also failed on 5 other PR(s): #7410, #7488, #7516, #7517, #7585; failed on 2 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 1: `features/login/impl/src/main/kotlin/io/element/android/features/login/impl/screens/confirmaccountprovider/ConfirmAccountProviderView.kt: ConfirmAccountProviderView`

### #7516 `io.element.android.tests.konsist.KonsistClassNameTest` (:tests:konsist:testDebugUnitTest)

- Merge commit `f740595ad5e4c86786e41a2fc53e317a036a91ec`, base `beddb3e86e9b8efe0cb7ad9560e5eea82a911b4a` (merge)
- Missed by: kt-affected, kt-affected+assist; attribution **c** (auto-flake)
- Test file: `tests/konsist/src/test/kotlin/io/element/android/tests/konsist/KonsistClassNameTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 32734086591 attempt 1 (log, 57201539bb)
- Flake evidence: also failed on 5 other PR(s): #7410, #7488, #7517, #7521, #7585; failed on 2 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 0

### #7585 `io.element.android.tests.konsist.KonsistClassNameTest` (:tests:konsist:testDebugUnitTest)

- Merge commit `c204d867a2398fb18c4fef76fd26654aa56b8f76`, base `97126715c190e386b66c8520de88d0d8ef7058c0` (merge)
- Missed by: module-level; attribution **c** (auto-flake)
- Test file: `tests/konsist/src/test/kotlin/io/element/android/tests/konsist/KonsistClassNameTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 33103445305 attempt 1 (log, 178a4b4cbd), run 33118972049 attempt 1 (log, 712fe5b217)
- Flake evidence: also failed on 4 other PR(s): #7410, #7516, #7517, #7521; failed on 2 base-branch push run(s)
- Changed declarations the test file names: none
- Functional changed declarations: 3: `app/src/main/kotlin/io/element/android/x/MainActivity.kt: MainActivity.onCreate`; `app/src/main/kotlin/io/element/android/x/di/AppBindings.kt: AppBindings.appStartupHooks`; `features/enterprise/api/src/main/kotlin/io/element/android/features/enterprise/api/AppStartupHook.kt: (whole file)`

### #7692 `io.element.android.tests.konsist.KonsistPreviewTest` (:tests:konsist:testDebugUnitTest)

- Merge commit `60000bfa5886a6ab43386e02403fc9dc3b5ae6dc`, base `f5ed93b4742fc64c533c2c84df43d508ec8095a2` (merge)
- Missed by: module-level; attribution **b** (hand)
- Test file: `tests/konsist/src/test/kotlin/io/element/android/tests/konsist/KonsistPreviewTest.kt`; changed in the PR: no; in kt-affected's inventory: yes
- Failed on: run 34591253106 attempt 1 (log, d59120aad0), run 34592591421 attempt 1 (log, c643acda38)
- Flake evidence: none
- Changed declarations the test file names: none
- Functional changed declarations: 20: `appnav/src/main/kotlin/io/element/android/appnav/LoggedInFlowNode.kt: LoggedInFlowNode.resolve`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: SyncOrchestrator`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: SyncOrchestrator.restartAttempt`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: SyncOrchestrator.observeStates`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: FIRST_RESTART_DELAY`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: RESTART_BACKOFF_DELAYS`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: RESTART_BACKOFF_RESET_DELAY`; `appnav/src/main/kotlin/io/element/android/appnav/session/SyncOrchestrator.kt: restartDelay`
- Hand check: Konsist scans the project's source files. The rule failed on the PR's own new code and the PR's next commit fixed it ("Use PreviewParameterProvider", on the PR's new previews); no push to the base branch failed it. A source-scanning test observes every source file, so no declaration path reaches it.

