use super::{Harbour, Host, Intent, Lighting};

#[test]
fn an_idle_update_preserves_the_game_state_without_claiming_an_action() {
    let mut host = Host::new(Harbour::initial(), Lighting::SpendOil);
    host.app.update();
    assert_eq!(host.state().unwrap(), &Harbour::initial());
    let intent = host.app.world().resource::<Intent>();
    assert_eq!(intent.pending, None);
    assert_eq!(intent.accepted, None);
}
