use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use bevy_ecs::event::{Event, EventCursor, Events};
use futures::{FutureExt, Stream};

use crate::{TaskContext, WithWorld};

pub trait EventStreamTaskExt {
    fn event_stream<E: Event + Clone + Unpin>(&self) -> impl Stream<Item = E>;
}

impl EventStreamTaskExt for TaskContext {
    fn event_stream<E: Event + Clone + Unpin>(&self) -> impl Stream<Item = E> {
        EventStream::<E>::new(self.clone())
    }
}

struct EventStreamData<E: Event> {
    items: VecDeque<E>,
    reader: EventCursor<E>,
}

impl<E: Event> Default for EventStreamData<E> {
    fn default() -> Self {
        EventStreamData {
            items: Default::default(),
            reader: Default::default(),
        }
    }
}

enum EventStreamState<E: Event> {
    HasItems(EventStreamData<E>),
    WaitingForTask(WithWorld<EventStreamData<E>>),
}

impl<E: Event> Default for EventStreamState<E> {
    fn default() -> Self {
        Self::HasItems(Default::default())
    }
}

/// Provides a [`Stream`] interface over a series of [`Event`]s. Asynchronously iterates
/// over all [`Event`]s from the start of the [`Events`] queue.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mod_async::prelude::*;
/// # use futures::StreamExt;
/// # #[derive(Event, Clone)]
/// # struct MyEvent(u32);
/// # App::new()
/// #     .add_plugins((MinimalPlugins, AssetPlugin::default(), AsyncTasksPlugin))
/// #     .add_event::<MyEvent>()
/// #     .add_systems(Main, |world: &mut World| {
/// world.spawn_task(|cx| async move {
///     let mut events = cx.event_stream::<MyEvent>();
///     assert!(matches!(events.next().await, Some(MyEvent(1))));
///     assert!(matches!(events.next().await, Some(MyEvent(2))));
///     assert!(matches!(events.next().await, Some(MyEvent(3))));
/// });
/// world.send_event(MyEvent(1));
/// world.send_event(MyEvent(2));
/// world.send_event(MyEvent(3));
/// # world.send_event(AppExit::Success);
/// #     })
/// #     .run();
/// ```
pub struct EventStream<E>
where
    E: Event,
{
    cx: TaskContext,
    state: EventStreamState<E>,
}

impl<E: Event> EventStream<E> {
    pub fn new(cx: TaskContext) -> Self {
        Self {
            cx,
            state: Default::default(),
        }
    }
}

impl<E: Event + Clone + Unpin> Stream for EventStream<E> {
    type Item = E;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.state {
                EventStreamState::HasItems(data) => {
                    if let Some(next) = data.items.pop_front() {
                        return Poll::Ready(Some(next));
                    } else {
                        let mut reader = std::mem::replace(&mut data.reader, Default::default());
                        let fut = self.cx.with_world(move |world| {
                            let items = reader
                                .read(world.resource::<Events<E>>())
                                .map(Clone::clone)
                                .collect::<VecDeque<_>>();
                            EventStreamData { items, reader }
                        });
                        self.state = EventStreamState::WaitingForTask(fut);
                    }
                }
                EventStreamState::WaitingForTask(fut) => {
                    if let Poll::Ready(data) = fut.poll_unpin(cx) {
                        self.state = EventStreamState::HasItems(data);
                    } else {
                        return Poll::Pending;
                    }
                }
            }
        }
    }
}
