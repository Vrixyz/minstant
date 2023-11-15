use bevy::prelude::*;

use bevy_http_client::*;
use serde::Serialize;
use serde::Deserialize;
pub struct ComPlugin;

impl Plugin for ComPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HttpClientPlugin);
        app.init_resource::<ApiTimer>();

        app.add_systems(Update, (send_login, handle_login));
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct ApiTimer(pub Timer);

impl Default for ApiTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Once))
    }
}

#[derive(Deserialize, Serialize)]
pub struct LoginData {
    pub name: String,
    pub password: String,
}

fn send_login(mut commands: Commands, time: Res<Time>, mut timer: ResMut<ApiTimer>) {
    timer.tick(time.delta());

    if timer.just_finished() {
        let data = serde_json::to_vec(&LoginData{name: "Test".to_string(), password: "test".to_string()}).unwrap();
        let mut req = ehttp::Request::post("http://127.0.0.1:8000/users/login", data);
        req
                .headers
                .insert("Content-Type".into(), "application/json".into());
        commands.spawn(HttpRequest(dbg!(req)));
    }
}

fn handle_login(mut commands: Commands, responses: Query<(Entity, &HttpResponse)>) {
    for (entity, response) in responses.iter() {
        info!("response: {:?}", response.headers);
        commands.entity(entity).despawn_recursive();
    }
}
