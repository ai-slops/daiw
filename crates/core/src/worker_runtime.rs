use crate::composition_cache::{CompositionCache, ResolutionSession};
use crate::composition_cache_metrics::CacheLimits;
use crate::error::CoreError;
use crate::protocol::{FailureResponse, Request, Response, SuccessResponse};
use serde_json::json;

pub struct WorkerRuntime {
    cache: CompositionCache,
}

impl WorkerRuntime {
    #[must_use]
    pub fn with_cache_limits(limits: CacheLimits) -> Self {
        Self {
            cache: CompositionCache::new(limits),
        }
    }

    pub fn dispatch(&mut self, request: Request) -> Response {
        if request.method == "project.cacheStats" {
            return self.cache_stats(request);
        }
        self.cache.begin_request();
        let response = crate::dispatch::dispatch_with_session(
            request,
            &mut ResolutionSession::new(&mut self.cache),
        );
        self.cache.finish_request();
        response
    }

    fn cache_stats(&self, request: Request) -> Response {
        let id = request.id;
        let result = (|| -> Result<_, CoreError> {
            if request.principal != crate::protocol::Principal::Human {
                return Err(CoreError::PermissionDenied);
            }
            let state = request.state.ok_or(CoreError::MissingState)?;
            Ok((state, json!(self.cache.stats())))
        })();
        match result {
            Ok((state, result)) => Response::Success(Box::new(SuccessResponse {
                id,
                ok: true,
                state,
                result,
            })),
            Err(error) => Response::Failure(FailureResponse {
                id,
                ok: false,
                error: error.into(),
            }),
        }
    }
}

impl Default for WorkerRuntime {
    fn default() -> Self {
        Self::with_cache_limits(CacheLimits::default())
    }
}
