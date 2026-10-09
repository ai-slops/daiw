use crate::composition_cache::{CompositionCache, ResolutionSession};
use crate::composition_cache_metrics::CacheLimits;
use crate::error::CoreError;
use crate::protocol::{FailureResponse, Request, Response, SuccessResponse};
use serde_json::json;

pub struct WorkerRuntime {
    cache: CompositionCache,
    instance_cache: crate::instance_pattern_cache::InstancePatternCache,
}

impl WorkerRuntime {
    #[must_use]
    pub fn with_cache_limits(limits: CacheLimits) -> Self {
        Self::with_separate_cache_limits(limits, limits)
    }

    #[must_use]
    pub fn with_separate_cache_limits(
        source_limits: CacheLimits,
        instance_limits: CacheLimits,
    ) -> Self {
        Self {
            cache: CompositionCache::new(source_limits),
            instance_cache: crate::instance_pattern_cache::InstancePatternCache::new(
                instance_limits,
            ),
        }
    }

    pub fn dispatch(&mut self, request: Request) -> Response {
        if request.method == "project.cacheStats" {
            return self.cache_stats(request, self.cache.stats());
        }
        if request.method == "project.instanceCacheStats" {
            return self.cache_stats(request, self.instance_cache.stats());
        }
        self.cache.begin_request();
        self.instance_cache.begin_request();
        let response = crate::dispatch::dispatch_with_session(
            request,
            &mut ResolutionSession::with_pattern_cache(&mut self.cache, &mut self.instance_cache),
        );
        self.cache.finish_request();
        self.instance_cache.finish_request();
        response
    }

    fn cache_stats(
        &self,
        request: Request,
        stats: crate::composition_cache_metrics::CacheStats,
    ) -> Response {
        let id = request.id;
        let result = (|| -> Result<_, CoreError> {
            if request.principal != crate::protocol::Principal::Human {
                return Err(CoreError::PermissionDenied);
            }
            let state = request.state.ok_or(CoreError::MissingState)?;
            Ok((state, json!(stats)))
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
