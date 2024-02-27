use std::{convert::Infallible, error::Error, fmt::{self, Debug}};

use digest::{typenum::U32, Digest, OutputSizeUser};
use futures::{ stream, Stream, StreamExt};

use crate::{types::{handle::Handle, Hash}, Blob, BlobParseError, Bloblike, BlobSet};

#[derive(Debug)]
pub enum TransferError<ListErr, LoadErr, StoreErr> {
    List(ListErr),
    Load(LoadErr),
    Store(StoreErr)
}

impl<ListErr, LoadErr, StoreErr> fmt::Display for TransferError<ListErr, LoadErr, StoreErr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to transfer blob")
    }
}

impl<ListErr, LoadErr, StoreErr> Error for TransferError<ListErr, LoadErr, StoreErr>
where ListErr: Debug + Error + 'static,
      LoadErr: Debug + Error + 'static,
      StoreErr: Debug + Error + 'static  {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::List(e) => Some(e),
            Self::Load(e)  => Some(e),
            Self::Store(e) => Some(e)
        }
    }
}

async fn transfer<'a, BS, BT, HS, HT, S>(source: &'a BS, target: &'a BT) -> impl Stream<Item = Result<(Hash<HS>, Hash<HT>), TransferError<BS::ListErr, BS::LoadErr, BT::StoreErr>>> + 'a
where BS: BlobPull<HS>,
      BT: BlobPush<HT>,
      HS: 'static,
      HT: 'static {
    let l = source.list();
    let r = l.then(move |source_hash:Result<Hash<HS>, <BS as BlobPull<HS>>::ListErr>| {
        async move {
            let source_hash = source_hash.map_err(|e| TransferError::List(e))?;
            let blob = source.pull_raw(source_hash).await
                .map_err(|e| TransferError::Load(e))?;
            let target_hash = target.push_raw(blob).await
                .map_err(|e| TransferError::Store(e))?;
            Ok((source_hash, target_hash))
        }
    });
    r
}

#[derive(Debug)]
enum GetError<E> {
    Load(E),
    Parse(BlobParseError),
}

pub trait BlobPull<H> {
    type LoadErr;
    type ListErr;

    fn list<'a>(&'a self) -> impl Stream<Item = Result<Hash<H>, Self::ListErr>>;

    async fn pull_raw(&self, hash: Hash<H>) -> Result<Blob, Self::LoadErr>;
    
    async fn pull<T>(&self, handle: Handle<H, T>) -> Result<T, GetError<Self::LoadErr>>
    where T: Bloblike {
        let blob = self.pull_raw(handle.hash).await
        .map_err(|e| GetError::Load(e))?;
        T::from_blob(blob).map_err(|e| GetError::Parse(e))
    }
}


pub trait BlobPush<H> {
    type StoreErr;
    
    async fn push_raw(&self, blob: Blob) -> Result<Hash<H>, Self::StoreErr>;
    
    async fn push<T>(&self, value: T) -> Result<Handle<H, T>, Self::StoreErr>
    where T: Bloblike {
        let blob: Blob = value.into_blob();
        let hash = self.push_raw(blob).await?;
        Ok(unsafe{ Handle::new(hash) })
    }
}

pub trait BlobRepo<H>: BlobPull<H> + BlobPush<H> {}

#[derive(Debug)]
pub struct NotFoundErr();

impl fmt::Display for NotFoundErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no blob for hash in blobset")
    }
}

impl Error for NotFoundErr {}

impl<H> BlobPull<H> for BlobSet<H>
where H: Digest + OutputSizeUser<OutputSize = U32> {
    type LoadErr = NotFoundErr;
    type ListErr = Infallible;

    fn list<'a>(&'a self) -> impl Stream<Item = Result<Hash<H>, Self::ListErr>> {
        stream::iter((&self).into_iter().map(|(hash, _)| Ok(hash)))
    }

    async fn pull_raw(&self, hash: Hash<H>) -> Result<Blob, Self::LoadErr> {
        self.get_raw(hash).map_or(
            Err(NotFoundErr()),
            |b| Ok(b.clone()))
    }
}
