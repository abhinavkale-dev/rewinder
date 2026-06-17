#ifndef REWINDER_FFI_H
#define REWINDER_FFI_H

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct RewinderHandle RewinderHandle;

typedef void (*RewinderEventCallback)(const char *event,
                                      const char *json,
                                      void *ctx);

RewinderHandle *rewinder_init(void);

void rewinder_set_event_callback(RewinderHandle *handle,
                                 RewinderEventCallback callback,
                                 void *ctx);

void rewinder_shutdown(RewinderHandle *handle);

void rewinder_free_string(char *ptr);

char *rewinder_get_engine_state(RewinderHandle *handle);
char *rewinder_get_settings(RewinderHandle *handle);
char *rewinder_default_settings(void);
char *rewinder_update_settings(RewinderHandle *handle, const char *patch_json);
char *rewinder_set_replay_enabled(RewinderHandle *handle, bool enabled);
char *rewinder_resume_capture(RewinderHandle *handle);
char *rewinder_trigger_save_replay(RewinderHandle *handle, const char *source_json);
char *rewinder_list_recent_clips(RewinderHandle *handle, size_t limit);
char *rewinder_recheck_permissions(RewinderHandle *handle);
char *rewinder_request_microphone_permission(RewinderHandle *handle);
char *rewinder_list_microphones(void);
char *rewinder_grant_output_dir_access(RewinderHandle *handle);
char *rewinder_grant_screen_recording_access(RewinderHandle *handle);
char *rewinder_grant_microphone_access(RewinderHandle *handle, bool open_settings_if_denied);

#ifdef __cplusplus
}
#endif

#endif
