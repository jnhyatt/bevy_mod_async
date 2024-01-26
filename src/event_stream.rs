use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use bevy_ecs::event::{Event, Events, ManualEventReader};
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
    reader: ManualEventReader<E>,
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
/// let mut events = cx.event_stream::<KeyboardEvent>();
/// while let Some(ev) = events.next().await {
///     println!("Got a keyboard event: {ev:?}");
/// }
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
                    }
                }
            }
        }
    }
}
