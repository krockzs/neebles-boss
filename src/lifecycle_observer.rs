use crate::lifecycle_communication::LifecycleCommunication;

use std::sync::Arc;

/*
 * Lifecycle live observation boundary.
 *
 * This contract is deliberately transport-neutral.
 *
 * It does not know:
 * - Boss UI
 * - IPC
 * - Launcher
 * - Tray
 * - Qt
 * - sockets
 * - module technology
 *
 * Lifecycle publishes immutable communication snapshots.
 *
 * Observer failure is intentionally impossible at this boundary:
 * observation must never change execution semantics.
 */

pub type LifecycleObserverHandler = Arc<dyn Fn(LifecycleCommunication) + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub struct LifecycleObserver {
    handler: Option<LifecycleObserverHandler>,
}

impl LifecycleObserver {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn new(handler: LifecycleObserverHandler) -> Self {
        Self {
            handler: Some(handler),
        }
    }

    pub fn observing<F>(handler: F) -> Self
    where
        F: Fn(LifecycleCommunication) + Send + Sync + 'static,
    {
        Self::new(Arc::new(handler))
    }

    pub fn is_attached(&self) -> bool {
        self.handler.is_some()
    }

    pub fn publish(&self, communication: &LifecycleCommunication) {
        let Some(handler) = &self.handler else {
            return;
        };

        handler(communication.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle_state::LifecycleRuntimeState;

    use std::sync::{Arc, Mutex};

    fn communication() -> LifecycleCommunication {
        let state = LifecycleRuntimeState::new("execution-observer-test", "module.alpha").unwrap();

        LifecycleCommunication::from_state(&state).unwrap()
    }

    #[test]
    fn detached_observer_is_valid() {
        let observer = LifecycleObserver::none();

        assert!(!observer.is_attached());

        observer.publish(&communication());
    }

    #[test]
    fn attached_observer_receives_snapshot() {
        let received = Arc::new(Mutex::new(Vec::<LifecycleCommunication>::new()));

        let sink = Arc::clone(&received);

        let observer = LifecycleObserver::observing(move |snapshot| {
            sink.lock().unwrap().push(snapshot);
        });

        let mut communication = communication();

        communication.put("progress.completed", "1").unwrap();

        observer.publish(&communication);

        let received = received.lock().unwrap();

        assert_eq!(received.len(), 1);

        assert_eq!(received[0].get("progress.completed"), Some("1"));
    }

    #[test]
    fn observer_receives_owned_snapshot_not_live_alias() {
        let received = Arc::new(Mutex::new(Vec::<LifecycleCommunication>::new()));

        let sink = Arc::clone(&received);

        let observer = LifecycleObserver::observing(move |snapshot| {
            sink.lock().unwrap().push(snapshot);
        });

        let mut communication = communication();

        communication.put("state.phase", "running").unwrap();

        observer.publish(&communication);

        communication.put("state.phase", "finished").unwrap();

        let received = received.lock().unwrap();

        assert_eq!(received[0].get("state.phase"), Some("running"));

        assert_eq!(communication.get("state.phase"), Some("finished"));
    }

    #[test]
    fn arbitrary_future_namespaces_survive_observation() {
        let received = Arc::new(Mutex::new(Vec::<LifecycleCommunication>::new()));

        let sink = Arc::clone(&received);

        let observer = LifecycleObserver::observing(move |snapshot| {
            sink.lock().unwrap().push(snapshot);
        });

        let mut communication = communication();

        communication.put("future.anything", "banana").unwrap();

        observer.publish(&communication);

        assert_eq!(
            received.lock().unwrap()[0].get("future.anything"),
            Some("banana")
        );
    }
}
