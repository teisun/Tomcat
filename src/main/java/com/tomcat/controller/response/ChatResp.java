package com.tomcat.controller.response;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.unfbx.chatgpt.entity.common.Usage;
import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: ChatResp
 * @author: tomcat
 * @description: chat 回答
 * @date: 2023/8/30 1:36 PM
 * @version: 1.0
 */
@Data
public class ChatResp<T> {
    int code;
    String command;
    String describe;
    T data;
    Usage usage;
}
