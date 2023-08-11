package com.tomcat.controller.requeset;

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
