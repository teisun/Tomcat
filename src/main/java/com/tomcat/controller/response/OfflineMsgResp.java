package com.tomcat.controller.response;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: OfflineMsgResp
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/11 3:21 PM
 * @version: 1.0
 */
@Data
public class OfflineMsgResp {
    private String chatId;
    private String msg;
}
