//! The contract types cross a channel between a worker thread and the GUI, so every
//! one of them must be `Send` and clonable. This is the C2 channel's precondition.

use negpy_backend::event::{
    BackendCommand, BackendEvent, CameraSetting, CaptureLevels, ConnectionStatus,
    DensitometerReading, DiscoveryRequest, NormalizationRequest, PickBetterFrames, PixelReadout,
    PreviewFrame, PreviewLoadRequest, Progress, RenderMetrics, RenderParams, RenderResult,
    ThumbnailRenderRequest, ThumbnailUpdateRequest,
};

fn assert_send_clone<T: Send + Clone>() {}

#[test]
fn every_contract_type_is_send_and_clone() {
    assert_send_clone::<BackendEvent>();
    assert_send_clone::<BackendCommand>();
    assert_send_clone::<Progress>();
    assert_send_clone::<CaptureLevels>();
    assert_send_clone::<RenderMetrics>();
    assert_send_clone::<PreviewFrame>();
    assert_send_clone::<RenderResult>();
    assert_send_clone::<PickBetterFrames>();
    assert_send_clone::<PixelReadout>();
    assert_send_clone::<DensitometerReading>();
    assert_send_clone::<ConnectionStatus>();
    assert_send_clone::<CameraSetting>();
    assert_send_clone::<RenderParams>();
    assert_send_clone::<PreviewLoadRequest>();
    assert_send_clone::<NormalizationRequest>();
    assert_send_clone::<ThumbnailRenderRequest>();
    assert_send_clone::<ThumbnailUpdateRequest>();
    assert_send_clone::<DiscoveryRequest>();
}
