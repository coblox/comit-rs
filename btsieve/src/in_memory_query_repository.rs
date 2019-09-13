use crate::query_repository::{Error, QueryRepository};
use std::{collections::HashMap, sync::RwLock};

#[derive(Debug)]
struct State<T> {
    storage: HashMap<String, T>,
}

#[derive(Debug)]
pub struct InMemoryQueryRepository<Q> {
    state: RwLock<State<Q>>,
}

impl<Q> Default for InMemoryQueryRepository<Q> {
    fn default() -> Self {
        Self {
            state: RwLock::new(State {
                storage: HashMap::new(),
            }),
        }
    }
}

impl<T: Send + Sync + Clone + 'static> QueryRepository<T> for InMemoryQueryRepository<T> {
    fn all(&self) -> Box<dyn Iterator<Item = (String, T)>> {
        let state = self.state.read().unwrap();

        Box::new(state.storage.clone().into_iter())
    }

    fn get(&self, id: String) -> Option<T> {
        let state = self.state.read().unwrap();

        state.storage.get(&id).cloned()
    }

    fn save_with_id(&self, entity: T, id: String) -> Result<String, Error<T>> {
        let mut state = self.state.write().unwrap();

        state.storage.insert(id.clone(), entity);

        Ok(id)
    }

    fn delete(&self, id: String) {
        let mut state = self.state.write().unwrap();

        state.storage.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectral::prelude::*;

    #[derive(Debug, PartialEq, Clone)]
    struct MyEntity;

    #[test]
    fn given_entity_when_inserted_can_be_retrieved_with_generated_id() {
        let repository = InMemoryQueryRepository::default();

        let id = repository.save(MyEntity);

        assert_that(&id).is_ok();
        assert_that(&repository.get(id.unwrap()))
            .is_some()
            .is_equal_to(&MyEntity);
    }

    #[test]
    fn given_entity_when_deleted_is_no_longer_there() {
        let repository = InMemoryQueryRepository::default();

        let id = repository.save(MyEntity).unwrap();
        repository.delete(id.clone());

        assert_that(&repository.get(id)).is_none()
    }
}
