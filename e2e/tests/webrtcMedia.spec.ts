/**
 * 真实媒体门禁：需后端、前端（Vite 或同源静态资源）、ZLM 和已注册且能推 RTP 的设备/Mock。
 * 运行：E2E_REQUIRE_WEBRTC=1 npx playwright test tests/webrtcMedia.spec.ts
 * 同源部署另设 E2E_BASE_URL=http://127.0.0.1:18080 E2E_API_PREFIX=/api。
 * 分别检查 SDP 协商、ICE 连通及浏览器实际解码帧数。
 */
import { expect, test } from '@playwright/test';
import { apiPath } from '../test-helpers/api-path';

const DEVICE = '34020000001320000001';
const CHANNEL = '34020000001320000001';

test('真实 ZLM 流在浏览器完成 WebRTC ICE 并解出视频帧', async ({ page }) => {
  test.skip(process.env.E2E_REQUIRE_WEBRTC !== '1', '需要在线 GB28181 设备或 RTP Mock、真实 ZLM 及可达的 RTC 端口');
  test.setTimeout(90_000);

  await page.goto('/#/live', { waitUntil: 'domcontentloaded' });
  const token = await page.evaluate(() => {
    const value = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/)?.[1] ?? '';
    return decodeURIComponent(value);
  });
  expect(token).toBeTruthy();

  const headers = { 'access-token': token };
  const path = apiPath(`/play/start/${DEVICE}/${CHANNEL}`);
  const started = await page.request.get(path, { headers, timeout: 30_000 });
  const startBody = await started.json() as { code: number; msg?: string };
  expect(startBody.code, startBody.msg).toBe(0);

  try {
    const result = await page.evaluate(async ({ deviceId, channelId, accessToken, webrtcPath }) => {
      const pc = new RTCPeerConnection({ iceServers: [] });
      try {
        pc.addTransceiver('video', { direction: 'recvonly' });
        pc.addTransceiver('audio', { direction: 'recvonly' });
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        const response = await fetch(webrtcPath, {
          method: 'POST',
          headers: { 'content-type': 'application/json', 'access-token': accessToken },
          body: JSON.stringify({ deviceId, channelId, sdp: offer.sdp, type: 'offer' }),
        });
        const body = await response.json() as {
          code: number;
          msg?: string;
          data?: { sdp?: string };
        };
        if (body.code !== 0 || !body.data?.sdp) {
          throw new Error(`SDP 协商失败: ${body.msg ?? body.code}`);
        }
        await pc.setRemoteDescription({ type: 'answer', sdp: body.data.sdp });

        const deadline = Date.now() + 25_000;
        let framesDecoded = 0;
        let bytesReceived = 0;
        while (Date.now() < deadline) {
          const stats = await pc.getStats();
          stats.forEach((entry) => {
            if (entry.type === 'inbound-rtp' && entry.kind === 'video') {
              framesDecoded = Math.max(framesDecoded, entry.framesDecoded ?? 0);
              bytesReceived = Math.max(bytesReceived, entry.bytesReceived ?? 0);
            }
          });
          if ((pc.iceConnectionState === 'connected' || pc.iceConnectionState === 'completed')
            && framesDecoded > 0) break;
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
        return { ice: pc.iceConnectionState, framesDecoded, bytesReceived };
      } finally {
        pc.close();
      }
    }, { deviceId: DEVICE, channelId: CHANNEL, accessToken: token, webrtcPath: apiPath('/play/webrtc') });

    expect(['connected', 'completed']).toContain(result.ice);
    expect(result.bytesReceived).toBeGreaterThan(0);
    expect(result.framesDecoded).toBeGreaterThan(0);
    console.log(`[webrtc] ICE=${result.ice} frames=${result.framesDecoded} bytes=${result.bytesReceived}`);
  } finally {
    const stop = await page.request.get(apiPath(`/play/stop/${DEVICE}/${CHANNEL}`), { headers });
    const body = await stop.json() as { code: number; msg?: string };
    expect(body.code, body.msg).toBe(0);
  }
});
