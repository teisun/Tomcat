package com.tomcat.controller.requeset;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.requeset
 * @className: ChatUserData
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/5 2:58 PM
 * @version: 1.0
 */

@Data
public class ChatUserDataReq {
    private String user_sentence;
    private String prompt;
}
