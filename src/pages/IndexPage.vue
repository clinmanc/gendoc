<script lang="ts" setup>
import { reactive, ref, toRef } from 'vue';
import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { Event, listen } from '@tauri-apps/api/event';
import { useQuasar } from 'quasar';

const $q = useQuasar();

const form = reactive({
  path: '',
  msgType: 'sms',
  submitType: ['template'],
  eventMode: ['push'],
});

const loading = ref(false);

const filename = toRef(() => form.path.replace(/^.*[\\/]/, ''));

listen('tauri://file-drop', (event: Event<string[]>) => {
  if (event.payload.length > 0) {
    form.path = event.payload[0];
  }
});

async function pickFile() {
  const selected = await open();
  if (Array.isArray(selected)) {
    form.path = selected.length > 0 ? selected[0] : '';
  } else if (selected !== null) {
    form.path = selected;
  }
}

function removeFile() {
  form.path = '';
}

async function generate() {
  loading.value = true;
  try {
    await invoke('generate', form);
    $q.notify({
      icon: 'done',
      message: '生成成功',
      actions: [
        {
          label: '打开目录',
          handler: async () => {
            await invoke('open_directory', { path: form.path });
          },
        },
      ],
    });
  } catch (e) {
    $q.notify({
      type: 'negative',
      icon: 'error',
      message: String(e),
    });
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <q-page padding>
    <q-form class="flex flex-center" @submit="generate">
      <div
        v-if="!form.path"
        class="column items-center q-mt-xl q-pa-md q-gutter-md"
      >
        <q-btn
          label="选取模板文件"
          size="xl"
          color="primary"
          style="width: 320px"
          @click="pickFile"
        />
        <div class="text-caption">
          或者把一个模板文件拖动到这里。需要安装 Microsoft Word 或者 金山 Office
        </div>
      </div>
      <q-splitter
        v-else
        class="fullscreen"
        :model-value="50"
        :limits="[30, 50]"
      >
        <template #before>
          <div class="fit flex flex-center">
            <q-card v-ripple flat bordered style="width: 160px">
              <q-responsive :ratio="1">
                <q-icon color="primary" name="file_present" size="100px" />
              </q-responsive>
              <q-card-section class="text-center ellipsis">
                {{ filename }}
              </q-card-section>
              <div class="absolute-top-right q-pa-sm text-center">
                <q-btn dense round icon="close" @click="removeFile" />
              </div>
            </q-card>
          </div>
        </template>

        <template #after>
          <div class="q-col-gutter-md q-pa-md">
            <div class="text-h4">定制</div>
            <div>
              消息类型:
              <div class="q-pa-sm">
                <q-btn-toggle
                  v-model="form.msgType"
                  :options="[
                    { label: '文本短信', value: 'sms' },
                    { label: '视频短信', value: 'mms' },
                    { label: '国际短信', value: 'intl_sms' },
                  ]"
                  dense
                  spread
                />
              </div>
            </div>
            <div>
              提交方式:
              <q-option-group
                v-model="form.submitType"
                :options="[
                  { label: '文本', value: 'text' },
                  { label: '模板', value: 'template' },
                ]"
                type="checkbox"
                inline
              />
            </div>
            <div>
              事件模式:
              <q-option-group
                v-model="form.eventMode"
                :options="[
                  { label: '拉取', value: 'poll' },
                  { label: '推送', value: 'push' },
                ]"
                type="checkbox"
                inline
              />
            </div>

            <q-toolbar class="absolute-bottom q-pa-md">
              <q-btn
                class="full-width"
                type="submit"
                label="生成"
                color="primary"
                size="xl"
                :loading="loading"
              >
                <template #loading>
                  <q-spinner-facebook />
                </template>
              </q-btn>
            </q-toolbar>
          </div>
        </template>
      </q-splitter>
    </q-form>
  </q-page>
</template>
