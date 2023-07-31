package com.tomcat.controller.request;

import lombok.Data;

/**
 * 描述：
 *
 */
@Data
public class ChatRequest {
    /**
     * 客户端发送的问题参数
     */
    private String msg;
}
