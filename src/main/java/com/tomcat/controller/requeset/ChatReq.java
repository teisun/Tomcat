package com.tomcat.controller.requeset;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.requeset
 * @className: ChatReq
 * @author: tomcat
 * @description: 移动客户端发送的chat参数
 * @date: 2023/8/30 1:29 PM
 * @version: 1.0
 */
@Data
public class ChatReq {
    String uid;
    String command;
    String data;

}
