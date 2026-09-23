// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_installer_gui::page::{complete, confirm, disk, options, running, welcome};
use ghaf_setup_core::disk::BlockDevice;
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use ghaf_setup_ui::Page as PageTrait;

fn device(path: &str, removable: bool) -> BlockDevice {
    BlockDevice {
        path: path.into(),
        size_bytes: 500_107_862_016,
        model: Some("Test Disk".into()),
        removable,
        read_only: false,
    }
}

#[test]
fn the_first_disk_is_preselected() {
    let page = disk::Page::new(vec![device("/dev/sda", false), device("/dev/sdb", false)]);
    assert!(page.completed());
    assert_eq!(page.selected().unwrap().path, "/dev/sda");
}

#[test]
fn removable_drives_sort_after_fixed_disks() {
    let page = disk::Page::new(vec![
        device("/dev/sda", true),
        device("/dev/nvme0n1", false),
    ]);
    assert_eq!(page.selected().unwrap().path, "/dev/nvme0n1");
}

#[test]
fn choosing_a_disk_selects_it() {
    let mut page = disk::Page::new(vec![device("/dev/sda", false), device("/dev/sdb", false)]);
    page.select(1);
    assert_eq!(page.selected().unwrap().path, "/dev/sdb");
}

#[test]
fn new_with_selection_reselects_by_path() {
    let page = disk::Page::new_with_selection(
        vec![device("/dev/sda", false), device("/dev/sdb", false)],
        Some("/dev/sdb"),
        None,
    );
    assert_eq!(page.selected().unwrap().path, "/dev/sdb");
}

#[test]
fn new_with_selection_drops_a_vanished_device() {
    let page =
        disk::Page::new_with_selection(vec![device("/dev/sda", false)], Some("/dev/sdb"), None);
    assert!(page.selected().is_none());
}

#[test]
fn an_out_of_range_selection_is_ignored() {
    let mut page = disk::Page::new(vec![device("/dev/sda", false)]);
    page.select(5);
    assert_eq!(page.selected().unwrap().path, "/dev/sda");
}

#[test]
fn removable_media_is_flagged_for_the_view() {
    let mut page = disk::Page::new(vec![device("/dev/sda", true)]);
    page.select(0);
    assert!(
        page.selected_is_removable(),
        "selecting the installer's own media is the worst mistake available"
    );
}

#[test]
fn an_empty_disk_list_cannot_proceed() {
    let page = disk::Page::new(vec![]);
    assert!(!page.completed());
}

#[test]
fn both_options_default_to_off() {
    let page = options::Page::new(true);
    assert!(!page.encrypt());
    assert!(!page.secure_boot());
}

#[test]
fn secure_boot_cannot_be_enabled_outside_setup_mode() {
    let mut page = options::Page::new(false);
    page.set_secure_boot(true);
    assert!(
        !page.secure_boot(),
        "enrollment that the firmware will reject must not be offered"
    );
}

#[test]
fn secure_boot_can_be_enabled_in_setup_mode() {
    let mut page = options::Page::new(true);
    page.set_secure_boot(true);
    assert!(page.secure_boot());
}

#[test]
fn set_setup_mode_keeps_an_existing_secure_boot_choice() {
    let mut page = options::Page::new(true);
    page.set_secure_boot(true);
    page.set_setup_mode(true);
    assert!(page.secure_boot(), "a late refresh must not discard it");
}

#[test]
fn set_setup_mode_clears_secure_boot_when_leaving_setup_mode() {
    let mut page = options::Page::new(true);
    page.set_secure_boot(true);
    page.set_setup_mode(false);
    assert!(!page.secure_boot());
}

#[test]
fn the_options_page_is_always_completable() {
    assert!(
        options::Page::new(false).completed(),
        "both options are optional"
    );
}

#[test]
fn no_action_is_chosen_initially() {
    let page = welcome::Page::default();
    assert!(page.action().is_none());
    assert!(!page.completed());
}

#[test]
fn choosing_an_action_unblocks_next() {
    let mut page = welcome::Page::default();
    page.choose(welcome::Action::Erase);
    assert_eq!(page.action(), Some(welcome::Action::Erase));
    assert!(page.completed());
}

#[test]
fn the_welcome_page_navigates_by_its_own_buttons() {
    assert!(!welcome::Page::default().show_next());
}

#[test]
fn the_start_page_links_to_erasing_and_a_shell() {
    let labels: Vec<_> = welcome::Page::default()
        .links()
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels, ["Erase a disk", "Exit to shell"]);
}

#[test]
fn the_start_page_offers_restart_and_shut_down() {
    assert!(welcome::Page::default().footer_end().is_some());
}

fn summary() -> confirm::Summary {
    confirm::Summary {
        action: "Install Ghaf".into(),
        wipe_only: false,
        device: "/dev/sda".into(),
        encrypt: true,
        secure_boot: false,
    }
}

#[test]
fn the_confirm_page_is_destructive() {
    let page = confirm::Page::new(summary());
    assert!(
        page.destructive(),
        "the primary action erases a disk and must be styled accordingly"
    );
}

#[test]
fn the_confirm_button_names_the_action() {
    assert_eq!(confirm::Page::new(summary()).next_label(), "Install");
    let wipe = confirm::Summary {
        wipe_only: true,
        ..summary()
    };
    assert_eq!(confirm::Page::new(wipe).next_label(), "Erase");
}

#[test]
fn erasing_is_confirmed_as_an_erase() {
    let erase = confirm::Summary {
        wipe_only: true,
        ..summary()
    };
    assert_eq!(confirm::Page::new(erase).title(), "Confirm erase");
    assert_eq!(
        confirm::Page::new(summary()).title(),
        "Confirm installation"
    );
}

#[test]
fn the_summary_names_the_target_device() {
    let page = confirm::Page::new(summary());
    assert!(page.summary_lines().iter().any(|l| l.contains("/dev/sda")));
}

#[test]
fn starts_running_and_blocks_next() {
    let page = running::Page::default();
    assert!(matches!(page.state(), running::RunState::Running));
    assert!(
        !page.completed(),
        "the user cannot skip past a running install"
    );
}

#[test]
fn a_finished_last_phase_succeeds() {
    let mut page = running::Page::default();
    for phase in [Phase::Wipe, Phase::WriteImage, Phase::BootEntry] {
        page.apply(ProgressEvent::PhaseStarted(phase));
        page.apply(ProgressEvent::PhaseFinished(phase));
    }
    page.finish();
    assert!(matches!(page.state(), running::RunState::Succeeded));
    assert!(page.completed());
}

#[test]
fn a_failure_records_whether_retry_is_safe() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::PhaseStarted(Phase::WriteImage));
    page.apply(ProgressEvent::Failed {
        phase: Phase::WriteImage,
        message: "short write".into(),
        recoverable: false,
    });
    assert!(matches!(
        page.state(),
        running::RunState::Failed {
            recoverable: false,
            disk_untouched: false
        }
    ));
    assert!(
        page.completed(),
        "the user must be able to reach the result page"
    );
}

#[test]
fn a_failure_before_any_phase_starts_is_reported_as_disk_untouched() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::Failed {
        phase: Phase::Wipe,
        message: "not a valid device path".into(),
        recoverable: false,
    });
    assert!(matches!(
        page.state(),
        running::RunState::Failed {
            recoverable: false,
            disk_untouched: true
        }
    ));
}

#[test]
fn reset_returns_to_running_so_a_retry_can_succeed() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::Failed {
        phase: Phase::SecureBoot,
        message: "efi-updatevar failed".into(),
        recoverable: true,
    });
    page.reset();
    assert!(matches!(page.state(), running::RunState::Running));
    page.finish();
    assert!(matches!(page.state(), running::RunState::Succeeded));
}

#[test]
fn each_step_moves_from_pending_to_running_to_done() {
    let mut page = running::Page::default();
    page.set_plan(vec![Phase::Wipe, Phase::WriteImage, Phase::BootEntry]);
    assert_eq!(page.step(Phase::Wipe), running::Step::Pending);

    page.apply(ProgressEvent::PhaseStarted(Phase::Wipe));
    assert_eq!(page.step(Phase::Wipe), running::Step::Running);
    assert_eq!(page.step(Phase::WriteImage), running::Step::Pending);

    page.apply(ProgressEvent::PhaseFinished(Phase::Wipe));
    page.apply(ProgressEvent::PhaseStarted(Phase::WriteImage));
    assert_eq!(page.step(Phase::Wipe), running::Step::Done);
    assert_eq!(page.step(Phase::WriteImage), running::Step::Running);
}

#[test]
fn there_is_no_going_back_from_a_run_or_its_result() {
    assert!(!running::Page::default().show_back());
    assert!(!running::Page::default().show_next());
    assert!(!complete_page(running::RunState::Succeeded, false).show_back());
}

#[test]
fn only_the_planned_steps_are_listed() {
    let mut page = running::Page::default();
    page.set_plan(vec![Phase::Wipe]);
    assert_eq!(page.plan(), [Phase::Wipe]);
}

#[test]
fn log_lines_are_bounded() {
    let mut page = running::Page::default();
    for i in 0..5000 {
        page.apply(ProgressEvent::Log(format!("line {i}")));
    }
    assert!(
        page.log().len() <= running::MAX_LOG_LINES,
        "an unbounded log would grow without limit"
    );
}

fn complete_page(state: running::RunState, erase: bool) -> complete::Page {
    complete::Page::new(state, erase, "/dev/sda".into(), None)
}

#[test]
fn an_install_ends_on_the_power_buttons_alone() {
    let page = complete_page(running::RunState::Succeeded, false);
    assert_eq!(page.title(), "Installation complete");
    assert!(!page.show_next(), "rebooting is the restart button's job");
    assert!(page.footer_end().is_some());
}

#[test]
fn an_erase_ends_by_returning_to_the_start() {
    let page = complete_page(running::RunState::Succeeded, true);
    assert_eq!(page.title(), "Disk erased");
    assert!(page.show_next());
    assert_eq!(page.finish_label(), "Back to start");
    assert!(page.starts_over());
    assert!(page.footer_end().is_some());
}

#[test]
fn a_failure_is_titled_as_a_failure() {
    let failed = running::RunState::Failed {
        recoverable: false,
        disk_untouched: false,
    };
    assert_eq!(complete_page(failed, false).title(), "Installation failed");
    assert_eq!(complete_page(failed, true).title(), "Erase failed");
}

#[test]
fn a_boot_failure_does_not_claim_a_partial_write() {
    let failed = running::RunState::Failed {
        recoverable: false,
        disk_untouched: false,
    };
    let boot = complete::Page::new(
        failed,
        false,
        "/dev/sda".into(),
        Some((Phase::BootEntry, "no EFI system partition".into())),
    );
    assert!(boot.message().contains("was written to /dev/sda"));
    assert!(!boot.message().contains("partway"));

    let write = complete::Page::new(
        failed,
        false,
        "/dev/sda".into(),
        Some((Phase::WriteImage, "short write".into())),
    );
    assert!(write.message().contains("partway"));
}

#[test]
fn a_failure_records_its_phase_and_message() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::PhaseStarted(Phase::BootEntry));
    page.apply(ProgressEvent::Failed {
        phase: Phase::BootEntry,
        message: "no EFI system partition".into(),
        recoverable: false,
    });
    assert_eq!(
        page.failure(),
        Some((Phase::BootEntry, "no EFI system partition".to_string()))
    );
}

#[test]
fn erasing_shows_an_erase_title_while_running() {
    let mut page = running::Page::default();
    page.set_erase(true);
    assert_eq!(page.title(), "Erasing disk");
}
