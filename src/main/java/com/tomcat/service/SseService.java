package com.tomcat.service;

import com.tomcat.controller.request.ChatRequest;
import com.tomcat.controller.response.ChatResponse;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

/**
 * 描述：
 *
 */
public interface SseService {
    /**
     * 创建SSE
     * @param uid
     * @return
     */
    SseEmitter createSse(String uid);

    /**
     * 关闭SSE
     * @param uid
     */
    void closeSse(String uid);

    /**
     * 客户端发送消息到服务端
     * @param uid
     * @param chatRequest
     */
    ChatResponse sseChat(String uid, ChatRequest chatRequest);
}
