package com.tomcat.controller.requeset;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.requeset
 * @className: TipsReq
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/7 10:51 AM
 * @version: 1.0
 */

@Data
public class TipsReq {
    private String topic;
    private String question;
}
