package com.tomcat.service;

import com.tomcat.controller.requeset.AuthenticationReq;
import com.tomcat.controller.response.AuthenticationResp;
import org.hamcrest.Matchers;
import org.junit.Assert;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.junit4.SpringRunner;

import javax.annotation.Resource;

@SpringBootTest
@RunWith(SpringRunner.class)
class UserServiceTest {

    @Resource
    private UserService userService;

    @Test
    void registerOrLogin() {
        AuthenticationReq request = new AuthenticationReq("tom", "1234", "", "13823232232", "8888888");
        AuthenticationResp authenticationResp = userService.registerOrLogin(request);
        System.out.println("registerOrLogin!");
        System.out.println("AccessToken= "+ authenticationResp.getAccessToken());
        Assert.assertThat(authenticationResp, Matchers.notNullValue());
    }
}