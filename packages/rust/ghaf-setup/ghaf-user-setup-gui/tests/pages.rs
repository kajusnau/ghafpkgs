// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_ui::Page as PageTrait;
use ghaf_user_setup_gui::page::account::{Message, Page};

fn filled(fido: bool) -> Page {
    let mut page = Page::new(fido);
    page.update(Message::Username("alice".into()));
    page.update(Message::RealName("Alice Liddell".into()));
    page.update(Message::Password("correct horse".into()));
    page.update(Message::Confirm("correct horse".into()));
    page.set_username_taken("alice", false);
    page
}

#[test]
fn a_complete_form_can_proceed() {
    let page = filled(false);
    assert!(page.completed());
    let request = page.request().unwrap();
    assert_eq!(request.username, "alice");
    assert!(!request.fido);
}

#[test]
fn an_empty_form_cannot_proceed_and_shows_no_errors_yet() {
    let page = Page::new(false);
    assert!(!page.completed());
    assert!(page.errors().username.is_none(), "no nagging before typing");
}

#[test]
fn characters_a_username_can_never_have_are_refused() {
    let mut page = filled(false);
    page.update(Message::Username("al ice".into()));
    page.update(Message::Username("alicé".into()));
    assert_eq!(page.username(), "alice", "typed or pasted, they never land");
}

#[test]
fn capitals_are_lowercased() {
    let mut page = filled(false);
    page.update(Message::Username("Alice".into()));
    assert_eq!(page.username(), "alice");
}

#[test]
fn a_badly_formed_username_is_explained_as_it_is_typed() {
    let mut page = filled(false);
    page.update(Message::Username("2alice".into()));
    assert!(!page.completed());
    assert!(page.errors().username.unwrap().contains("Start with"));
}

#[test]
fn passwd_separators_are_refused_in_the_full_name() {
    let mut page = filled(false);
    page.update(Message::RealName("Alice:Liddell".into()));
    assert_eq!(page.request().unwrap().real_name, "Alice Liddell");
}

#[test]
fn a_taken_username_blocks_until_checked_free() {
    let mut page = filled(false);
    page.update(Message::Username("root".into()));
    assert!(!page.completed(), "not yet known to be free");
    page.set_username_taken("root", true);
    assert!(page.errors().username.unwrap().contains("taken"));
    page.set_username_taken("alice", false);
    assert!(
        !page.completed(),
        "an answer for another name does not count"
    );
}

#[test]
fn mismatched_passwords_block() {
    let mut page = filled(false);
    page.update(Message::Confirm("correct hors".into()));
    assert!(!page.completed());
    assert!(page.errors().confirm.is_some());
}

#[test]
fn fido_is_only_offered_with_a_token_and_needs_no_pin() {
    let mut page = filled(false);
    page.update(Message::ToggleFido(true));
    assert!(!page.request().unwrap().fido, "no token, no FIDO");

    let mut page = filled(true);
    page.update(Message::ToggleFido(true));
    let request = page.request().unwrap();
    assert!(request.fido);
    assert_eq!(request.pin, None, "an empty PIN means the token has none");
}

#[test]
fn secrets_are_cleared_once_handed_over() {
    let mut page = filled(false);
    page.clear_secrets();
    assert!(page.request().is_none());
}

use ghaf_setup_core::homed::RecoveryKey;
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use ghaf_user_setup_gui::page::running::{self, Outcome};

#[test]
fn finish_waits_for_the_recovery_key_to_be_acknowledged() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::PhaseStarted(Phase::CreateAccount));
    page.apply(ProgressEvent::PhaseFinished(Phase::CreateAccount));
    page.conclude(Ok(Some(RecoveryKey("fhkbl-rtgvb".into()))));

    assert!(matches!(page.outcome(), Outcome::Created { key: Some(_) }));
    assert!(page.show_next(), "Finish is there");
    assert!(!page.completed(), "but disabled until acknowledged");
    page.update(running::Message::Acknowledge(true));
    assert!(page.completed());
    assert!(
        !page.show_back(),
        "an account cannot be un-created from here"
    );
}

#[test]
fn without_a_recovery_key_finish_is_immediate() {
    let mut page = running::Page::default();
    page.conclude(Ok(None));
    assert!(page.completed());
}

#[test]
fn a_failure_offers_back_to_the_form() {
    let mut page = running::Page::default();
    page.apply(ProgressEvent::Failed {
        phase: Phase::CreateAccount,
        message: "Password too weak".into(),
        recoverable: true,
    });
    page.conclude(Err("homectl failed".into()));
    assert_eq!(page.outcome(), Outcome::Failed("Password too weak".into()));
    assert_eq!(page.title(), "Account creation failed");
    assert!(!page.show_next());
    assert!(page.show_back(), "Back sits where the wizard always has it");
}

#[test]
fn nothing_can_be_pressed_while_creating() {
    let page = running::Page::default();
    assert!(!page.show_next() && !page.show_back());
    assert_eq!(page.outcome(), Outcome::Running);
}
