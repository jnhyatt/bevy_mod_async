use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use bevy_ecs::message::{Message, MessageCursor, Messages};
use futures::{FutureExt, Stream};

use crate::{TaskContext, WithWorld};

pub trait MessageStreamTaskExt {
    fn message_stream<M: Message + Clone + Unpin>(&self) -> impl Stream<Item = M>;
}

impl MessageStreamTaskExt for TaskContext {
    fn message_stream<M: Message + Clone + Unpin>(&self) -> impl Stream<Item = M> {
        MessageStream::<M>::new(self.clone())
    }
}

struct MessageStreamData<M: Message> {
    items: VecDeque<M>,
    reader: MessageCursor<M>,
}

impl<M: Message> Default for MessageStreamData<M> {
    fn default() -> Self {
        MessageStreamData {
            items: Default::default(),
            reader: Default::default(),
        }
    }
}

enum MessageStreamState<M: Message> {
    HasItems(MessageStreamData<M>),
    WaitingForTask(WithWorld<MessageStreamData<M>>),
}

impl<M: Message> Default for MessageStreamState<M> {
    fn default() -> Self {
        Self::HasItems(Default::default())
    }
}

/// Provides a [`Stream`] interface over a series of [`Message`]s. Asynchronously iterates
/// over all [`Message`]s from the start of the [`Messages`] queue.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mod_async::prelude::*;
/// # use futures::StreamExt;
/// # #[derive(Message, Clone)]
/// # struct MyMessage(u32);
/// # App::new()
/// #     .add_plugins((MinimalPlugins, AssetPlugin::default(), AsyncTasksPlugin))
/// #     .add_message::<MyMessage>()
/// #     .add_systems(Main, |world: &mut World| {
/// world.spawn_task(|cx| async move {
///     let mut messages = cx.message_stream::<MyMessage>();
///     assert!(matches!(messages.next().await, Some(MyMessage(1))));
///     assert!(matches!(messages.next().await, Some(MyMessage(2))));
///     assert!(matches!(messages.next().await, Some(MyMessage(3))));
/// });
/// world.write_message(MyMessage(1));
/// world.write_message(MyMessage(2));
/// world.write_message(MyMessage(3));
/// # world.write_message(AppExit::Success);
/// #     })
/// #     .run();
/// ```
pub struct MessageStream<M>
where
    M: Message,
{
    cx: TaskContext,
    state: MessageStreamState<M>,
}

impl<M: Message> MessageStream<M> {
    pub fn new(cx: TaskContext) -> Self {
        Self {
            cx,
            state: Default::default(),
        }
    }
}

impl<M: Message + Clone + Unpin> Stream for MessageStream<M> {
    type Item = M;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.state {
                MessageStreamState::HasItems(data) => {
                    if let Some(next) = data.items.pop_front() {
                        return Poll::Ready(Some(next));
                    } else {
                        let mut reader = std::mem::take(&mut data.reader);
                        let fut = self.cx.with_world(move |world| {
                            let items = reader
                                .read(world.resource::<Messages<M>>())
                                .map(Clone::clone)
                                .collect::<VecDeque<_>>();
                            MessageStreamData { items, reader }
                        });
                        self.state = MessageStreamState::WaitingForTask(fut);
                    }
                }
                MessageStreamState::WaitingForTask(fut) => {
                    if let Poll::Ready(data) = fut.poll_unpin(cx) {
                        self.state = MessageStreamState::HasItems(data);
                    } else {
                        return Poll::Pending;
                    }
                }
            }
        }
    }
}
